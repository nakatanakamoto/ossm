#![no_std]

use core::{
    fmt::Write,
    sync::atomic::{AtomicBool, Ordering},
};

pub const CONNECTIONS_MAX: usize = 1;
pub const L2CAP_CHANNELS_MAX: usize = 2;
pub const MAX_COMMAND_LENGTH: usize = 64;
pub const MAX_STATE_LENGTH: usize = 128;
pub const MAX_PATTERN_LENGTH: usize = 256;

use embassy_executor::Spawner;
use embassy_futures::select::{Either, select};
use embassy_time::{Duration, Ticker, Timer};
use esp_radio::ble::controller::BleConnector;
use heapless::String;
use log::{error, info};
use pattern_engine::PatternEngine;
use pattern_engine::{AnyPattern, Pattern};
use static_cell::StaticCell;
use trouble_host::prelude::*;

const SERVICE_UUID: Uuid = uuid!("522b443a-4f53-534d-0001-420badbabe69");
const PRIMARY_COMMAND_UUID: Uuid = uuid!("522b443a-4f53-534d-1000-420badbabe69");
const SPEED_KNOB_UUID: Uuid = uuid!("522b443a-4f53-534d-1010-420badbabe69");
const CURRENT_STATE_UUID: Uuid = uuid!("522b443a-4f53-534d-2000-420badbabe69");
const PATTERN_LIST_UUID: Uuid = uuid!("522b443a-4f53-534d-3000-420badbabe69");
const PATTERN_DESCRIPTION_UUID: Uuid = uuid!("522b443a-4f53-534d-3010-420badbabe69");

static CONNECTED: AtomicBool = AtomicBool::new(false);

macro_rules! mk_static {
    ($t:ty, $val:expr) => {{
        static STATIC_CELL: StaticCell<$t> = StaticCell::new();
        STATIC_CELL.init($val)
    }};
}

#[gatt_server]
struct Server {
    ossm_service: OssmService,
}

#[gatt_service(uuid = SERVICE_UUID)]
struct OssmService {
    #[characteristic(uuid = PRIMARY_COMMAND_UUID, read, write)]
    primary_command: String<MAX_COMMAND_LENGTH>,

    #[characteristic(uuid = SPEED_KNOB_UUID, read, write)]
    speed_knob_characteristic: String<16>,

    #[characteristic(uuid = CURRENT_STATE_UUID, read, notify)]
    current_state: String<MAX_STATE_LENGTH>,

    #[characteristic(uuid = PATTERN_LIST_UUID, read)]
    pattern_list: String<MAX_PATTERN_LENGTH>,

    #[characteristic(uuid = PATTERN_DESCRIPTION_UUID, read, write)]
    pattern_description: String<MAX_PATTERN_LENGTH>,
}

/// Returns all patterns as json
pub fn get_all_patterns_json() -> String<MAX_PATTERN_LENGTH> {
    let patterns = pattern_engine::AnyPattern::all_builtin();

    let mut output = String::new();
    output.write_char('[').ok();
    for (i, pattern) in patterns.iter().enumerate() {
        let name = pattern.name();
        if write!(output, r#"{{"name":"{name}","idx":{i}}},"#).is_err() {
            error!("Patterns too long. Returning unfinished string");
            break;
        }
    }
    // Remove the last comma
    output.pop();

    if output.write_char(']').is_err() {
        error!("Patterns too long. Returning unfinished string");
    }

    output
}

pub fn get_pattern_description(index: usize) -> String<MAX_PATTERN_LENGTH> {
    let patterns = pattern_engine::AnyPattern::all_builtin();

    let mut output = String::new();

    let description = if let Some(pattern) = patterns.get(index) {
        pattern.description()
    } else {
        "Invalid pattern index"
    };

    if output.push_str(description).is_err() {
        output
            .push_str("Pattern Description Too Long")
            .expect("Always fits");
    }

    output
}

pub fn start(
    spawner: &Spawner,
    mut connector: BleConnector<'static>,
    engine: &'static PatternEngine,
) {
    let bt_controller: ExternalController<_, 20> = ExternalController::new(connector);

    let resources = mk_static!(HostResources<DefaultPacketPool, CONNECTIONS_MAX, L2CAP_CHANNELS_MAX>, HostResources::new());
    let stack = mk_static!(
        trouble_host::Stack<
            'static,
            ExternalController<BleConnector<'static>, 20>,
            DefaultPacketPool,
        >,
        trouble_host::new(bt_controller, resources)
    );

    let Host {
        peripheral, runner, ..
    } = stack.build();

    spawner.must_spawn(ble_runner_task(runner));
    spawner.must_spawn(ble_events_task(stack, peripheral, engine));

    info!("BLE remote tasks started, waiting for connection...");
}

#[embassy_executor::task]
pub async fn ble_events_task(
    stack: &'static Stack<
        'static,
        ExternalController<BleConnector<'static>, 20>,
        DefaultPacketPool,
    >,
    mut peripheral: Peripheral<
        'static,
        ExternalController<BleConnector<'static>, 20>,
        DefaultPacketPool,
    >,
    engine: &'static PatternEngine,
) {
    info!("Starting advertising and GATT service");
    let server = Server::new_with_config(GapConfig::Peripheral(PeripheralConfig {
        name: "OSSM",
        appearance: &appearance::motorized_device::GENERIC_MOTORIZED_DEVICE,
    }))
    .unwrap();

    loop {
        match advertise("OSSM", &mut peripheral).await {
            Ok(connection) => {
                CONNECTED.store(true, Ordering::Release);
                engine.play(0);
                info!("BLE Connected");

                Timer::after_millis(100).await;

                connection
                    .set_phy(stack, PhyKind::Le2M)
                    .await
                    .expect("Could not set 2M PHY");

                let connect_params = ConnectParams {
                    min_connection_interval: Duration::from_micros(7500),
                    max_connection_interval: Duration::from_micros(7500),
                    ..Default::default()
                };
                connection
                    .update_connection_params(stack, &connect_params)
                    .await
                    .expect("Failed to update connection params");

                Timer::after_millis(100).await;

                let phy = connection.read_phy(stack).await.unwrap();
                let mtu = connection.att_mtu();
                info!("PHY {:?} MTU {:?}", phy, mtu);

                let gatt_connection = connection
                    .with_attribute_server(&server)
                    .expect("Could not transform connection into GATT connection");

                let events = gatt_events_task(&server, &gatt_connection, engine);
                let notify = state_notifications(&server, &gatt_connection);

                match select(events, notify).await {
                    Either::First(res) => {
                        if let Err(err) = res {
                            panic!("[gatt] error in events task: {:?}", err);
                        }
                    }
                    Either::Second(res) => {
                        if let Err(err) = res {
                            panic!("[gatt] error in notify task: {:?}", err);
                        }
                    }
                }
            }
            Err(err) => {
                panic!("[adv] error: {:?}", err);
            }
        }
    }
}

#[embassy_executor::task]
pub async fn ble_runner_task(
    mut runner: Runner<'static, ExternalController<BleConnector<'static>, 20>, DefaultPacketPool>,
) {
    loop {
        if let Err(err) = runner.run().await {
            panic!("[ble_task] error: {:?}", err);
        }
    }
}

async fn gatt_events_task<P: PacketPool>(
    server: &Server<'_>,
    connection: &GattConnection<'_, '_, P>,
    engine: &'static PatternEngine,
) -> Result<(), Error> {
    let reason = loop {
        match connection.next().await {
            GattConnectionEvent::Disconnected { reason } => break reason,
            GattConnectionEvent::Gatt { event } => {
                let mut write = false;
                let mut event_handle = 0;
                match &event {
                    GattEvent::Read(event) => {
                        if event.handle() == server.ossm_service.current_state.handle {
                            // let state: String<MAX_STATE_LENGTH> = get_motion_state().as_json();
                            // server.set(&server.ossm_service.current_state, &state)?;
                        }
                        if event.handle() == server.ossm_service.pattern_list.handle {
                            let patterns = get_all_patterns_json();
                            server.set(&server.ossm_service.pattern_list, &patterns)?;
                        }
                    }
                    GattEvent::Write(event) => {
                        write = true;
                        event_handle = event.handle();
                    }
                    _ => {}
                };
                // This step is also performed at drop(), but writing it explicitly is necessary
                // in order to ensure reply is sent.
                match event.accept() {
                    Ok(reply) => reply.send().await,
                    Err(e) => {
                        error!("[gatt] error sending response: {:?}", e);
                    }
                };

                // This is here because the event needs to be accepted before the data can be accessed
                if write {
                    if event_handle == server.ossm_service.primary_command.handle {
                        let command: String<MAX_COMMAND_LENGTH> =
                            server.get(&server.ossm_service.primary_command)?;

                        process_command(&command, server, engine);
                    }
                    if event_handle == server.ossm_service.pattern_description.handle {
                        let command: String<MAX_PATTERN_LENGTH> =
                            server.get(&server.ossm_service.pattern_description)?;

                        let description = if let Ok(index) = command.parse::<usize>() {
                            get_pattern_description(index)
                        } else {
                            let mut description: String<MAX_PATTERN_LENGTH> = String::new();
                            description
                                .push_str("Could not parse pattern index")
                                .expect("Always fits");
                            description
                        };

                        server.set(&server.ossm_service.pattern_description, &description)?;
                    }
                }
            }
            _ => {} // ignore other Gatt Connection Events
        }
    };
    CONNECTED.store(false, Ordering::Release);
    info!("[gatt] disconnected: {:?}", reason);
    Ok(())
}

/// Create an advertiser to use to connect to a BLE Central, and wait for it to connect.
async fn advertise<'values, 'server, C: Controller>(
    name: &'values str,
    peripheral: &mut Peripheral<'values, C, DefaultPacketPool>,
) -> Result<Connection<'values, DefaultPacketPool>, BleHostError<C::Error>> {
    let uuid: [u8; 16] = SERVICE_UUID
        .as_raw()
        .try_into()
        .expect("Service UUID incorrect");

    let mut advertiser_data = [0; 31];
    let len = AdStructure::encode_slice(
        &[
            AdStructure::Flags(LE_GENERAL_DISCOVERABLE | BR_EDR_NOT_SUPPORTED),
            AdStructure::ServiceUuids128(&[uuid]),
            AdStructure::CompleteLocalName(name.as_bytes()),
        ],
        &mut advertiser_data[..],
    )?;
    let advertiser = peripheral
        .advertise(
            &Default::default(),
            Advertisement::ConnectableScannableUndirected {
                adv_data: &advertiser_data[..len],
                scan_data: &[],
            },
        )
        .await?;
    info!("[adv] advertising");
    let conn = advertiser.accept().await?;
    info!("[adv] connection established");
    Ok(conn)
}

async fn state_notifications<P: PacketPool>(
    server: &Server<'_>,
    connection: &GattConnection<'_, '_, P>,
) -> Result<(), Error> {
    let mut ticker = Ticker::every(Duration::from_millis(500));
    loop {
        // let state: String<MAX_STATE_LENGTH> = get_motion_state().as_json();
        // server
        //     .ossm_service
        //     .current_state
        //     .notify(connection, &state)
        //     .await?;
        ticker.next().await;
    }
}

fn process_command(
    command: &String<MAX_COMMAND_LENGTH>,
    server: &Server<'_>,
    engine: &'static PatternEngine,
) {
    info!("BLE Command {}", command);

    let mut split_command = command.split(":");

    let mut fail = false;

    if let Some(cmd) = split_command.next() {
        if let Some(action) = split_command.next() {
            match cmd {
                "set" => {
                    if let Some(value) = split_command.next() {
                        if let Ok(value) = value.parse::<u32>() {
                            let clamped_value = (value as f64 / 100.0).clamp(0.0, 1.0);
                            match action {
                                "speed" => {
                                    engine.input().lock(|cell| {
                                        cell.update(|mut x| {
                                            x.velocity = clamped_value;
                                            x
                                        });
                                    });
                                }
                                "stroke" => {
                                    engine.input().lock(|cell| {
                                        cell.update(|mut x| {
                                            x.stroke = clamped_value;
                                            x
                                        });
                                    });
                                }
                                "depth" => {
                                    engine.input().lock(|cell| {
                                        cell.update(|mut x| {
                                            x.depth = clamped_value;
                                            x
                                        });
                                    });
                                }
                                "sensation" => {
                                    engine.input().lock(|cell| {
                                        cell.update(|mut x| {
                                            x.sensation = clamped_value;
                                            x
                                        });
                                    });
                                }
                                "pattern" => {
                                    engine.play(value as usize);
                                }
                                _ => {
                                    error!("Invalid set command {}", action);
                                    fail = true;
                                }
                            }
                        } else {
                            error!("Could not parse set value");
                            fail = true;
                        };
                    } else {
                        error!("No value after set");
                        fail = true;
                    }
                }
                "go" => match action {
                    "simplePenetration" => {}
                    "strokeEngine" => {}
                    "menu" => {}
                    _ => {
                        error!("Invalid go command {}", action);
                        fail = true;
                    }
                },
                _ => {
                    error!("Command neither set nor go");
                    fail = true;
                }
            }
        } else {
            error!("No action in command");
            fail = true;
        }
    } else {
        error!("Invalid command");
        fail = true;
    }

    let mut response_str: String<MAX_COMMAND_LENGTH> = String::new();
    if fail {
        response_str.write_str("fail:").expect("Should always fit");
        if response_str.write_str(command.as_str()).is_err() {
            response_str
                .write_str("overflow")
                .expect("Should always fit");
        }
    } else {
        response_str.write_str("ok:").expect("Should always fit");
        if response_str.write_str(command.as_str()).is_err() {
            response_str
                .write_str("overflow")
                .expect("Should always fit");
        }
    }
    if let Err(err) = server.set(&server.ossm_service.primary_command, &response_str) {
        error!("Failed to write the response to a set command {:?}", err);
    }
}

pub fn is_ble_connected() -> bool {
    CONNECTED.load(Ordering::Acquire)
}
