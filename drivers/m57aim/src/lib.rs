#![no_std]

use embedded_hal_async::delay::DelayNs;
use ossm::{Motor, SelfHomingMotor};

/// Async Modbus wire protocol. The only trait in the motor driver layer.
///
/// Implementations handle framing, CRC, and physical transport (RS485, TCP, etc.).
/// The motor struct uses this to read/write registers without knowing the wire details.
#[allow(async_fn_in_trait)]
pub trait ModbusTransport {
    type Error: core::fmt::Debug;

    /// Write a single holding register (function code 0x06).
    async fn write_holding(
        &mut self,
        device_addr: u8,
        register: u16,
        value: u16,
    ) -> Result<(), Self::Error>;

    /// Read one or more holding registers (function code 0x03).
    async fn read_holding(
        &mut self,
        device_addr: u8,
        register: u16,
        count: u16,
    ) -> Result<heapless::Vec<u16, 8>, Self::Error>;

    /// Send a raw frame and read the response.
    ///
    /// Used for vendor-specific function codes (e.g. the 57AIM's 0x7B
    /// absolute position command) that don't fit standard Modbus functions.
    async fn raw_transaction(
        &mut self,
        request: &[u8],
        response: &mut [u8],
    ) -> Result<usize, Self::Error>;
}

/// Modbus interface: wraps a transport + device address.
pub struct Modbus<T: ModbusTransport> {
    pub transport: T,
    pub device_addr: u8,
}

impl<T: ModbusTransport> Modbus<T> {
    pub fn new(transport: T, device_addr: u8) -> Self {
        Self {
            transport,
            device_addr,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(u16)]
pub enum RwRegister {
    ModbusEnable = 0x00,
    DriverOutputEnable = 0x01,
    MotorTargetSpeed = 0x02,
    MotorAcceleration = 0x03,
    WeakMagneticAngle = 0x04,
    SpeedRingProportionalCoefficient = 0x05,
    SpeedLoopIntegrationTime = 0x06,
    PositionRingProportionalCoefficient = 0x07,
    SpeedFeedForward = 0x08,
    DirPolarity = 0x09,
    ElectronicGearNumerator = 0x0A,
    ElectronicGearDenominator = 0x0B,
    ParameterSaveFlag = 0x14,
    AbsolutePositionLowU16 = 0x16,
    AbsolutePositionHighU16 = 0x17,
    StandstillMaxOutput = 0x18,
    SpecificFunction = 0x19,
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(u16)]
pub enum RoRegister {
    TargetPositionLowU16 = 0x0C,
    TargetPositionHighU16 = 0x0D,
    AlarmCode = 0x0E,
    SystemCurrent = 0x0F,
    MotorCurrentSpeed = 0x10,
    SystemVoltage = 0x11,
    SystemTemperature = 0x12,
    SystemOutputPwm = 0x13,
    DeviceAddress = 0x15,
}

pub struct Motor57AIMConfig {
    pub steps_per_rev: u32,
    pub max_output: u16,
}

impl Default for Motor57AIMConfig {
    fn default() -> Self {
        Self {
            steps_per_rev: 32_768,
            max_output: 600,
        }
    }
}

/// 57AIM BLDC servo motor, generic over communication interface and delay.
pub struct Motor57AIM<I, D> {
    pub interface: I,
    pub config: Motor57AIMConfig,
    pub delay: D,
}

impl<I, D> Motor57AIM<I, D> {
    pub fn new(interface: I, config: Motor57AIMConfig, delay: D) -> Self {
        Self {
            interface,
            config,
            delay,
        }
    }
}

mod modbus_constants {
    pub const SET_ABSOLUTE_POSITION_FUNC: u8 = 0x7B;
    pub const HOME_SPEED_RPM: u16 = 80;
    pub const HOME_MAX_OUTPUT: u16 = 89;
    pub const HOME_STEP_THRESHOLD: i32 = 15;
    pub const HOME_POLL_INTERVAL_MS: u32 = 50;
    pub const POST_HOME_SETTLE_MS: u32 = 20;
    pub const POST_ENABLE_SETTLE_MS: u32 = 800;
    pub const OPERATING_SPEED_RPM: u16 = 3000;
    pub const OPERATING_ACCELERATION: u16 = 50000;
    pub const OPERATING_MAX_OUTPUT: u16 = 600;
}

impl<T: ModbusTransport, D> Motor57AIM<Modbus<T>, D> {
    async fn write_register(&mut self, reg: RwRegister, value: u16) -> Result<(), T::Error> {
        self.interface
            .transport
            .write_holding(self.interface.device_addr, reg as u16, value)
            .await
    }

    async fn read_register(&mut self, reg: u16) -> Result<u16, T::Error> {
        let regs = self
            .interface
            .transport
            .read_holding(self.interface.device_addr, reg, 1)
            .await?;
        Ok(regs[0])
    }

    async fn read_register_pair(&mut self, reg_low: u16) -> Result<i32, T::Error> {
        let regs = self
            .interface
            .transport
            .read_holding(self.interface.device_addr, reg_low, 2)
            .await?;
        let raw = ((regs[1] as u32) << 16) | regs[0] as u32;
        Ok(raw as i32)
    }

    /// Enable Modbus control and driver output.
    pub async fn enable_driver(&mut self) -> Result<(), T::Error> {
        self.write_register(RwRegister::ModbusEnable, 1).await?;
        self.write_register(RwRegister::DriverOutputEnable, 1).await
    }

    /// Disable driver output and Modbus control.
    pub async fn disable_driver(&mut self) -> Result<(), T::Error> {
        self.write_register(RwRegister::DriverOutputEnable, 0).await?;
        self.write_register(RwRegister::ModbusEnable, 0).await
    }

    /// Atomic 4-byte absolute position command (vendor function 0x7B).
    pub async fn set_absolute_position(&mut self, steps: i32) -> Result<(), T::Error> {
        let mut request = [0u8; 6];
        request[0] = self.interface.device_addr;
        request[1] = modbus_constants::SET_ABSOLUTE_POSITION_FUNC;
        request[2..6].copy_from_slice(&steps.to_be_bytes());
        let mut response = [0u8; 8];
        self.interface
            .transport
            .raw_transaction(&request, &mut response)
            .await?;
        Ok(())
    }

    pub async fn set_speed(&mut self, rpm: u16) -> Result<(), T::Error> {
        self.write_register(RwRegister::MotorTargetSpeed, rpm).await
    }

    pub async fn set_acceleration(&mut self, value: u16) -> Result<(), T::Error> {
        self.write_register(RwRegister::MotorAcceleration, value)
            .await
    }

    pub async fn set_max_output(&mut self, output: u16) -> Result<(), T::Error> {
        self.write_register(RwRegister::StandstillMaxOutput, output)
            .await
    }

    /// Configure the motor for maximum tracking performance.
    ///
    /// Sets speed, acceleration, and output to maximum so the motor acts
    /// as a pure position servo, following commands from ruckig with
    /// minimal lag.
    pub async fn configure_max_tracking(&mut self) -> Result<(), T::Error> {
        self.set_speed(modbus_constants::OPERATING_SPEED_RPM).await?;
        self.set_acceleration(modbus_constants::OPERATING_ACCELERATION)
            .await?;
        self.set_max_output(modbus_constants::OPERATING_MAX_OUTPUT)
            .await
    }

    /// Trigger the motor's built-in homing sequence.
    pub async fn trigger_homing(&mut self) -> Result<(), T::Error> {
        self.write_register(RwRegister::MotorTargetSpeed, modbus_constants::HOME_SPEED_RPM)
            .await?;
        self.write_register(RwRegister::StandstillMaxOutput, modbus_constants::HOME_MAX_OUTPUT)
            .await?;
        self.write_register(RwRegister::SpecificFunction, 1).await
    }

    pub async fn read_absolute_position(&mut self) -> Result<i32, T::Error> {
        self.read_register_pair(RwRegister::AbsolutePositionLowU16 as u16)
            .await
    }

    pub async fn read_remaining_steps(&mut self) -> Result<i32, T::Error> {
        self.read_register_pair(RoRegister::TargetPositionLowU16 as u16)
            .await
    }

    pub async fn read_current_amps(&mut self) -> Result<f32, T::Error> {
        let raw = self.read_register(RoRegister::SystemCurrent as u16).await?;
        Ok(raw as f32 / 2000.0)
    }

    pub async fn read_voltage_volts(&mut self) -> Result<f32, T::Error> {
        let raw = self.read_register(RoRegister::SystemVoltage as u16).await?;
        Ok(raw as f32 / 327.0)
    }

    pub async fn read_temperature(&mut self) -> Result<u16, T::Error> {
        self.read_register(RoRegister::SystemTemperature as u16)
            .await
    }

    pub async fn read_alarm_code(&mut self) -> Result<u16, T::Error> {
        self.read_register(RoRegister::AlarmCode as u16).await
    }
}

impl<T: ModbusTransport, D: DelayNs> Motor for Motor57AIM<Modbus<T>, D> {
    type Error = T::Error;

    fn steps_per_rev(&self) -> u32 {
        self.config.steps_per_rev
    }

    fn max_output(&self) -> u16 {
        self.config.max_output
    }

    async fn enable(&mut self) -> Result<(), Self::Error> {
        self.enable_driver().await?;
        self.configure_max_tracking().await
    }

    async fn disable(&mut self) -> Result<(), Self::Error> {
        self.disable_driver().await
    }

    async fn set_absolute_position(&mut self, steps: i32) -> Result<(), Self::Error> {
        Motor57AIM::set_absolute_position(self, steps).await
    }

    async fn read_absolute_position(&mut self) -> Result<i32, Self::Error> {
        Motor57AIM::read_absolute_position(self).await
    }

    async fn set_max_output(&mut self, output: u16) -> Result<(), Self::Error> {
        Motor57AIM::set_max_output(self, output).await
    }
}

impl<T: ModbusTransport, D: DelayNs> SelfHomingMotor for Motor57AIM<Modbus<T>, D> {
    async fn home(&mut self) -> Result<(), Self::Error> {
        self.trigger_homing().await?;

        loop {
            self.delay.delay_ms(modbus_constants::HOME_POLL_INTERVAL_MS).await;
            let remaining = self.read_remaining_steps().await?;
            if remaining.abs() < modbus_constants::HOME_STEP_THRESHOLD {
                break;
            }
        }

        self.delay.delay_ms(modbus_constants::POST_HOME_SETTLE_MS).await;

        // Re-enable Modbus (homing resets it on the 57AIM)
        self.enable_driver().await?;
        self.delay.delay_ms(modbus_constants::POST_ENABLE_SETTLE_MS).await;

        self.configure_max_tracking().await
    }
}
