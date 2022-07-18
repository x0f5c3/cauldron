use std::cmp;
use std::num::Wrapping;

use crate::io::{BitStream, ReadBuffer};
use crate::{errors, Result};

/// For each sample in buffer value is same
/// https://xiph.org/flac/format.html#subframe_constant
pub fn decode_constant<R: ReadBuffer>(
    bitstream: &mut BitStream<R>,
    fr_bps: u32,
    buffer: &mut [i32],
) -> Result<()> {
    let sample = extend_sign_u32(bitstream.read_len_u32(fr_bps)?, fr_bps);

    for b in buffer.iter_mut() {
        *b = sample;
    }
    Ok(())
}

/// Samples are stored without any encoding
/// https://xiph.org/flac/format.html#subframe_verbatim
#[cold]
pub fn decode_verbatim<R: ReadBuffer>(
    bitstream: &mut BitStream<R>,
    fr_bps: u32,
    buffer: &mut [i32],
) -> Result<()> {
    for b in buffer.iter_mut() {
        *b = extend_sign_u32(bitstream.read_len_u32(fr_bps)?, fr_bps);
    }
    Ok(())
}

/// A prediction polynomial is used
/// https://xiph.org/flac/format.html#subframe_fixed
pub fn decode_fixed_linear<R: ReadBuffer>(
    bitstream: &mut BitStream<R>,
    fr_bps: u32,
    order: usize,
    buffer: &mut [i32],
) -> Result<()> {
    // The length of the buffer must be greater than order
    // because the number of warm-up samples is equal to order.
    if buffer.len() < order {
        return errors::parse_error("invalid fixed subframe, order is larger than block size");
    }
    // There are order * bits per sample unencoded warm-up sample bits.
    decode_verbatim(bitstream, fr_bps, &mut buffer[..order])?;

    // decode residual
    decode_residual(bitstream, buffer.len() as u16, &mut buffer[order..])?;

    // based on polynomial fix the samples
    fixed_predict(order, buffer)?;

    Ok(())
}

/// https://xiph.org/flac/format.html#subframe_lpc
pub fn decode_lpc<R: ReadBuffer>(
    bitstream: &mut BitStream<R>,
    fr_bps: u32,
    order: usize,
    buffer: &mut [i32],
) -> Result<()> {
    // The length of the buffer must be greater than order
    // because the number of warm-up samples is equal to order.
    if buffer.len() < order {
        return errors::parse_error("invalid lpc subframe, order is larger than block size");
    }
    // There are order * bits per sample unencoded warm-up sample bits.
    decode_verbatim(bitstream, fr_bps, &mut buffer[..order])?;

    let qlpc_precision = bitstream.read_len_u8(4)? as u32 + 1;
    if qlpc_precision > 15 {
        return errors::parse_error("invalid lpc subframe, qlpc value invalid");
    }
    let qlpc_shift = extend_sign_u16(bitstream.read_len_u8(5)? as u16, 5);

    // The spec does allow the qlp shift to be negative, but in real it happens
    // very less, hence not supported for now.
    if qlpc_shift < 0 {
        return errors::unsupported_error(
            "negative quantized linear predictor coefficient shift not supported",
        );
    }

    // Now read the lpc coefficients
    let mut coefficients = [0; 32];
    for coef in coefficients[..order].iter_mut().rev() {
        // We can safely read into a u16, qlpc_precision is at most 15.
        *coef = extend_sign_u16(bitstream.read_len_u16(qlpc_precision)?, qlpc_precision);
    }

    // decode residual
    decode_residual(bitstream, buffer.len() as u16, &mut buffer[order..])?;

    if order <= 12 {
        predict_lpc_low_order(&coefficients[..order], qlpc_shift, buffer);
    } else {
        predict_lpc_high_order(&coefficients[..order], qlpc_shift, buffer);
    }

    Ok(())
}

/// Given a signed two's complement integer in the `bits` least significant
/// bits of `val`, extends the sign bit to a valid 32-bit signed integer.
#[inline(always)]
fn extend_sign_u32(val: u32, bits: u32) -> i32 {
    // First shift the value so the desired sign bit is the actual sign bit,
    // then convert to a signed integer, and then do an arithmetic shift back,
    // which will extend the sign bit.
    ((val << (32 - bits)) as i32) >> (32 - bits)
}

/// Given a signed two's complement integer in the `bits` least significant
/// bits of `val`, extends the sign bit to a valid 16-bit signed integer.
#[inline(always)]
fn extend_sign_u16(val: u16, bits: u32) -> i16 {
    // First shift the value so the desired sign bit is the actual sign bit,
    // then convert to a signed integer, and then do an arithmetic shift back,
    // which will extend the sign bit.
    ((val << (16 - bits)) as i16) >> (16 - bits)
}

/// Decodes a signed number from Rice coding to the two's complement.
///
/// The Rice coding used by FLAC operates on unsigned integers, but the
/// residual is signed. The mapping is done as follows:
///
///  0 -> 0
/// -1 -> 1
///  1 -> 2
/// -2 -> 3
///  2 -> 4
///  etc.
///
/// This function takes the unsigned value and converts it into a signed
/// number.
#[inline(always)]
fn rice_to_signed(val: u32) -> i32 {
    // The following bit-level hackery compiles to only four instructions on
    // x64. It is equivalent to the following code:
    //
    //   if val & 1 == 1 {
    //       -1 - (val / 2) as i32
    //   } else {
    //       (val / 2) as i32
    //   }
    //
    let half = (val >> 1) as i32;
    let extended_bit_0 = ((val << 31) as i32) >> 31;
    half ^ extended_bit_0
}

#[test]
fn test_rice_to_signed() {
    assert_eq!(rice_to_signed(0), 0);
    assert_eq!(rice_to_signed(1), -1);
    assert_eq!(rice_to_signed(2), 1);
    assert_eq!(rice_to_signed(3), -2);
    assert_eq!(rice_to_signed(4), 2);
}

fn fixed_predict(order: usize, buffer: &mut [i32]) -> Result<()> {
    // The Fixed Predictor is just a hard-coded version of the Linear Predictor up to order 4 and
    // with fixed coefficients. Some cases may be simplified such as orders 0 and 1. For orders 2
    // through 4, use the same IIR-style algorithm as the Linear Predictor.
    match order {
        // A 0th order predictor always predicts 0, and therefore adds nothing to
        // any sample in buffer.
        0 => (),
        // A 1st order predictor always returns the previous sample since the polynomial is:
        // s(i) = 1*s(i),
        1 => {
            for i in 1..buffer.len() {
                buffer[i] += buffer[i - 1];
            }
        }
        // A 2nd order predictor uses the polynomial: s(i) = 2*s(i-1) - 1*s(i-2).
        2 => {
            for i in 2..buffer.len() {
                let a = Wrapping(-1) * Wrapping(i64::from(buffer[i - 2]));
                let b = Wrapping(2) * Wrapping(i64::from(buffer[i - 1]));
                buffer[i] += (a + b).0 as i32;
            }
        }
        // A 3rd order predictor uses the polynomial: s(i) = 3*s(i-1) - 3*s(i-2) + 1*s(i-3).
        3 => {
            for i in 3..buffer.len() {
                let a = Wrapping(1) * Wrapping(i64::from(buffer[i - 3]));
                let b = Wrapping(-3) * Wrapping(i64::from(buffer[i - 2]));
                let c = Wrapping(3) * Wrapping(i64::from(buffer[i - 1]));
                buffer[i] += (a + b + c).0 as i32;
            }
        }
        // A 4th order predictor uses the polynomial:
        // s(i) = 4*s(i-1) - 6*s(i-2) + 4*s(i-3) - 1*s(i-4).
        4 => {
            for i in 4..buffer.len() {
                let a = Wrapping(-1) * Wrapping(i64::from(buffer[i - 4]));
                let b = Wrapping(4) * Wrapping(i64::from(buffer[i - 3]));
                let c = Wrapping(-6) * Wrapping(i64::from(buffer[i - 2]));
                let d = Wrapping(4) * Wrapping(i64::from(buffer[i - 1]));
                buffer[i] += (a + b + c + d).0 as i32;
            }
        }
        _ => unreachable!(),
    };

    Ok(())
}

fn decode_residual<R: ReadBuffer>(
    bitstream: &mut BitStream<R>,
    block_size: u16,
    buffer: &mut [i32],
) -> Result<()> {
    let param_width = match bitstream.read_len_u8(2)? {
        0 => 4u32,
        1 => 5u32,
        _ => return errors::unsupported_error("Encountered reserved bits in residual"),
    };

    let partition_order = bitstream.read_len_u8(4)?;

    // there are at most 2^16 - 1 samples in the block
    let num_partitions = 1u32 << partition_order;

    // In general, all partitions have the same number of samples such that the
    // sum of all partition lengths equal the block length. Thus, the number of samples
    // in a partition can therefore be calculated with block_size / 2^order.
    let num_samples_per_partition = block_size >> partition_order;

    // total samples from each partition should be total block size
    // So block size should be multiple of 2^order
    if block_size & (num_partitions - 1) as u16 != 0 {
        return errors::parse_error("invalid partition order in residual");
    }
    let num_warm_up = block_size - buffer.len() as u16;

    // first partition contains (num_samples_per_partition - num of warm up samples) > 0
    // check for non negative first partition
    if num_warm_up > num_samples_per_partition {
        return errors::parse_error("invalid residual");
    }

    // finally decode rice on each 2^order partitions
    {
        let escape_param = (1 << param_width) - 1;
        let mut start = 0;
        let mut len = num_samples_per_partition - num_warm_up;
        for _ in 0..num_partitions {
            let rice_param = bitstream.read_len_u8(param_width)? as u32;
            decode_rice_partition(
                bitstream,
                rice_param,
                escape_param,
                &mut buffer[start..start + len as usize],
            )?;
            start += len as usize;
            len = num_samples_per_partition;
        }
    }

    Ok(())
}

fn decode_rice_partition<R: ReadBuffer>(
    bitstream: &mut BitStream<R>,
    rice_param: u32,
    escape_param: u32,
    buffer: &mut [i32],
) -> Result<()> {
    // If rice param is 1111 or 11111 then stream is rice encoded else
    // it is binary encoded.
    if rice_param < escape_param {
        // rice encoded
        //
        // Depending on the number of bits, at most two or three bytes need to be
        // read, so the code below is split into two cases for efficiency
        if rice_param <= 8 {
            for sample in buffer.iter_mut() {
                let q = bitstream.read_unary()?;
                let r = bitstream.read_len_u8(rice_param)? as u32;
                *sample = rice_to_signed((q << rice_param) | r);
            }
        } else if rice_param <= 16 {
            for sample in buffer.iter_mut() {
                let q = bitstream.read_unary()?;
                let r = bitstream.read_len_u16(rice_param)? as u32;
                *sample = rice_to_signed((q << rice_param) | r);
            }
        } else {
            for sample in buffer.iter_mut() {
                let q = bitstream.read_unary()?;
                let r = bitstream.read_len_u32(rice_param)?;
                *sample = rice_to_signed((q << rice_param) | r);
            }
        }
    } else {
        // binary encoded
        let residual_bits = bitstream.read_len_u8(5)? as u32;

        // Read each binary encoded residual and store in buffer.
        for sample in buffer.iter_mut() {
            *sample = extend_sign_u32(bitstream.read_len_u32(residual_bits)?, residual_bits);
        }
    }
    Ok(())
}

/// Apply LPC prediction for subframes with LPC order of at most 12.
///
/// This function takes advantage of the upper bound on the order. Virtually all
/// files that occur in the wild are subset-compliant files, which have an order
/// of at most 12, so it makes sense to optimize for this.
fn predict_lpc_low_order(raw_coefficients: &[i16], qlp_shift: i16, buffer: &mut [i32]) {
    // The decoded residuals are 25 bits at most (assuming subset FLAC of at
    // most 24 bits per sample, but there is the delta encoding for channels).
    // The coefficients are 16 bits at most, so their product is 41 bits. In
    // practice the predictor order does not exceed 12, so adding 12 numbers of
    // 41 bits each requires at most 53 bits. Therefore, do all intermediate
    // computations as i64.
    //
    // If the actual order is less than 12, simply set the early coefficients to 0.
    let order = raw_coefficients.len();
    let coefficients = {
        let mut buf = [0i64; 12];
        let mut i = 12 - order;
        for c in raw_coefficients {
            buf[i] = *c as i64;
            i += 1;
        }
        buf
    };

    // The linear prediction is essentially an inner product of the known
    // samples with the coefficients, followed by a shift. To be able to do an
    // inner product of 12 elements at a time, we must first have 12 samples.
    // If the predictor order is less, first predict the few samples after the
    // warm-up samples.
    let left = cmp::min(12, buffer.len()) - order;
    for i in 0..left {
        let prediction = raw_coefficients
            .iter()
            .zip(&buffer[i..order + i])
            .map(|(&c, &s)| c as i64 * s as i64)
            .sum::<i64>()
            >> qlp_shift;
        // adding linear prediction to residual decoded buffer
        buffer[order + i] = (prediction + buffer[order + i] as i64) as i32;
    }

    if buffer.len() <= 12 {
        return;
    }

    // At this point, buffer[0..12] has been predicted. For the rest of the
    // buffer we can do inner products of 12 samples. This reduces the amount of
    // conditional code, and improves performance significantly.
    let mut sum;
    for i in 12..buffer.len() {
        sum = 0;
        for j in 0..12 {
            sum += buffer[i - 12 + j] as i64 * coefficients[j]
        }
        // adding linear prediction to residual decoded buffer
        buffer[i] = ((sum >> qlp_shift) + buffer[i] as i64) as i32;
    }
}

#[test]
fn test_predict_lpc_low_order() {
    let coef = [-77, 164, -219, 146, 38, 161, -895, 1151];
    let shift = 9;
    let mut buffer = [
        3590, 3465, 2979, 2237, 1692, 1411, 900, 476, 188, -189, 49, 3, 37, 150, -353, -49,
    ];
    let result = [
        3590, 3465, 2979, 2237, 1692, 1411, 900, 476, 187, -255, -688, -1146, -1455, -1428, -1567,
        -1717,
    ];

    predict_lpc_low_order(&coef, shift, &mut buffer);

    assert_eq!(buffer, result);
}

/// Apply LPC prediction for non-subset subframes, with LPC order > 12.
fn predict_lpc_high_order(coefficients: &[i16], qlp_shift: i16, buffer: &mut [i32]) {
    // This function is a copy that lifts the order restrictions (and specializations)
    // at the cost of performance.

    let order = coefficients.len();

    // The linear prediction is essentially an inner product of the known
    // samples with the coefficients, followed by a shift. The first `order`
    // samples are stored as-is.
    for i in order..buffer.len() {
        let prediction = coefficients
            .iter()
            .zip(&buffer[i - order..i])
            .map(|(&c, &s)| c as i64 * s as i64)
            .sum::<i64>()
            >> qlp_shift;
        let delta = buffer[i] as i64;
        buffer[i] = (prediction + delta) as i32;
    }
}
