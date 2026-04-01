use esp_hal::{
    Blocking,
    analog::adc::{Adc, AdcPin},
    peripherals::{ADC1, GPIO36},
};
use ossm::CurrentSensor;

/// Reads motor current draw from an analog pin on ADC1.
///
/// ADC1 is used instead of ADC2 because ADC2 conflicts with the WiFi/BLE
/// radio on ESP32.
pub struct AdcCurrentSensor {
    adc: Adc<'static, ADC1<'static>, Blocking>,
    pin: AdcPin<GPIO36<'static>, ADC1<'static>>,
}

impl AdcCurrentSensor {
    pub fn new(adc: Adc<'static, ADC1<'static>, Blocking>, pin: AdcPin<GPIO36<'static>, ADC1<'static>>) -> Self {
        Self { adc, pin }
    }
}

impl CurrentSensor for AdcCurrentSensor {
    type Error = AdcError;

    async fn read_percent(&mut self) -> Result<f32, Self::Error> {
        let raw = nb::block!(self.adc.read_oneshot(&mut self.pin)).map_err(|_| AdcError)?;
        // 12-bit ADC: 0–4095 → 0.0–100.0%
        Ok(raw as f32 / 4095.0 * 100.0)
    }
}

#[derive(Debug)]
pub struct AdcError;
