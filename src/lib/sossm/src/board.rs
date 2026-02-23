use core::fmt::Debug;
use crate::Motor;

pub trait Board {
    type Error: Debug + From<<Self::M as Motor>::Error>;
    type M: Motor;

    fn motor(&mut self) -> &mut Self::M;
    fn steps_per_mm(&self) -> f32;

    fn enable(&mut self, enable: bool) -> Result<(), Self::Error> {
        self.motor().enable(enable)?;
        Ok(())
    }

    fn home(&mut self) -> Result<(), Self::Error> {
        self.motor().home()?;
        Ok(())
    }

    fn move_to(&mut self, mm: f32) -> Result<(), Self::Error> {
        let steps = (mm * self.steps_per_mm()) as i32;
        self.motor().set_absolute_position(steps)?;
        Ok(())
    }
}
