use super::Motor;

/// Trait for motors wired over RS-485.
#[allow(async_fn_in_trait)]
pub trait Rs485Motor: Motor {
    /// Set the motor's internal direction polarity.
    ///
    /// Affects the motor's built-in homing direction (and the external DIR pin
    /// interpretation, where applicable). Absolute-position commands use the
    /// motor's fixed encoder coordinates and are *not* affected — those still
    /// need to be sign-flipped at the board level.
    ///
    /// Default no-op for motors without a configurable polarity register.
    async fn set_dir_polarity(&mut self, _reverse: bool) -> Result<(), Self::Error> {
        Ok(())
    }
}
