use super::{errors, Result};

/// Converts a signed integer in the range -128-127 to an unsigned one in the range 0-255.
#[inline(always)]
pub fn u8_from_signed(x: i8) -> u8 {
    (x as i16 + 128) as u8
}

/// Tries to cast the sample to an 8-bit signed integer, returning an error on overflow.
#[inline(always)]
pub fn narrow_to_i8(x: i32) -> Result<i8> {
    if x < i8::MIN as i32 || x > i8::MAX as i32 {
        errors::parse_error::<i8>("Too Wide to cast to i8")
    } else {
        Ok(x as i8)
    }
}

#[test]
fn test_narrow_to_i8() {
    assert!(narrow_to_i8(127).is_ok());
    assert!(narrow_to_i8(128).is_err());
    assert!(narrow_to_i8(-128).is_ok());
    assert!(narrow_to_i8(-129).is_err());
}

/// Tries to cast the sample to a 16-bit signed integer, returning an error on overflow.
#[inline(always)]
pub fn narrow_to_i16(x: i32) -> Result<i16> {
    if x < i16::MIN as i32 || x > i16::MAX as i32 {
        errors::parse_error::<i16>("Too Wide to cast to i16")
    } else {
        Ok(x as i16)
    }
}

#[test]
fn test_narrow_to_i16() {
    assert!(narrow_to_i16(32767).is_ok());
    assert!(narrow_to_i16(32768).is_err());
    assert!(narrow_to_i16(-32768).is_ok());
    assert!(narrow_to_i16(-32769).is_err());
}

/// Tries to cast the sample to a 24-bit signed integer, returning an error on overflow.
#[inline(always)]
pub fn narrow_to_i24(x: i32) -> Result<i32> {
    if x < -(1 << 23) || x > (1 << 23) - 1 {
        errors::parse_error::<i32>("Too Wide to cast to i24")
    } else {
        Ok(x)
    }
}

#[test]
fn test_narrow_to_i24() {
    assert!(narrow_to_i24(8_388_607).is_ok());
    assert!(narrow_to_i24(8_388_608).is_err());
    assert!(narrow_to_i24(-8_388_608).is_ok());
    assert!(narrow_to_i24(-8_388_609).is_err());
}
