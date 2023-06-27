use embedded_graphics::{
    mono_font::MonoTextStyleBuilder,
    prelude::{DrawTarget, Point},
    text::{Baseline, Text, TextStyleBuilder, Alignment},
    Drawable,
};
use embedded_svc::{
    mqtt::client::{Connection, Event, Message, MessageImpl, QoS},
    utils::mqtt::client::ConnState,
    wifi::{AuthMethod, ClientConfiguration, Configuration},
};
#[cfg(feature = "epd2in9_v2")]
use epd_waveshare::epd2in9_v2::{Display2in9 as EpdDisplay, Epd2in9 as Epd};
#[cfg(feature = "epd2in9_v2")]
use epd_waveshare::prelude::DisplayRotation;
#[cfg(feature = "epd5in83_v2")]
use epd_waveshare::epd5in83_v2::{Display5in83 as EpdDisplay, Epd5in83 as Epd};
use epd_waveshare::prelude::{Color, WaveshareDisplay};
use esp_idf_hal::{
    delay::{Delay, Ets},
    gpio::{AnyIOPin, Gpio2, PinDriver},
    prelude::Peripherals,
    spi::{config::Config, SpiDeviceDriver, SpiDriverConfig},
};
use esp_idf_svc::{
    eventloop::EspSystemEventLoop,
    mqtt::client::{EspMqttClient, MqttClientConfiguration},
    nvs::EspDefaultNvsPartition,
    tls::X509,
    wifi::{BlockingWifi, EspWifi},
};
use esp_idf_sys::{self as _, EspError}; // If using the `binstart` feature of `esp-idf-sys`, always keep this module imported
use log::*;
use serde::Deserialize;
use std::{
    mem, slice,
    sync::mpsc::{self, Sender},
    thread,
    time::Duration,
};

#[cfg(feature = "epd2in9_v2")]
pub const WIFI_SSID: &str = "Wokwi-GUEST";
#[cfg(feature = "epd2in9_v2")]
pub const WIFI_PASS: &str = "";
#[cfg(feature = "epd5in83_v2")]
pub const WIFI_SSID: &str = ""; // FILL YOUR SSID HERE
#[cfg(feature = "epd5in83_v2")]
pub const WIFI_PASS: &str = ""; // FILL YOUR PW HERE

// for AWS IoT Core, be sure to use the mqtts protocol and use the -ats endpoint!
pub const MQTT_ENDPOINT: &str = "";
pub const MQTT_CLIENT_ID: &str = "esp32-epaper-main";
pub const MQTT_TOPIC_NAME: &str = "F3/P6";

// Display points
#[cfg(feature = "epd2in9_v2")]
pub const DISPLAY_CENTER: i32 = 148;
#[cfg(feature = "epd2in9_v2")]
pub const DISPLAY_END: i32 = 296;
#[cfg(feature = "epd2in9_v2")]
pub const FONT_HEIGHT: i32 = 13;

#[cfg(feature = "epd5in83_v2")]
pub const DISPLAY_CENTER: i32 = 324;
#[cfg(feature = "epd5in83_v2")]
pub const DISPLAY_END: i32 = 648;
#[cfg(feature = "epd5in83_v2")]
pub const FONT_HEIGHT: i32 = 20;

fn main() -> anyhow::Result<()> {
    // It is necessary to call this function once. Otherwise some patches to the runtime
    // implemented by esp-idf-sys might not link properly. See https://github.com/esp-rs/esp-idf-template/issues/71
    esp_idf_sys::link_patches();
    // Bind the log crate to the ESP Logging facilities
    esp_idf_svc::log::EspLogger::initialize_default();

    let peripherals = Peripherals::take().unwrap();
    let sys_loop = EspSystemEventLoop::take()?;
    let nvs = EspDefaultNvsPartition::take()?;

    Delay::delay_ms(3000);
    // Blocking so that we can block until the IP is obtained
    let mut wifi = BlockingWifi::wrap(
        EspWifi::new(peripherals.modem, sys_loop.clone(), Some(nvs))?,
        sys_loop,
    )?;

    configure_wifi(&mut wifi)?;

    Delay::delay_ms(3000);

    info!("Configuring the E-Ink display...");
    let mut display = EpdDisplay::default();

    let spi = peripherals.spi2;

    // Firebeetle pins
    let sclk = peripherals.pins.gpio18;
    let serial_out = peripherals.pins.gpio23;
    // Right now we assume the 2in9 screen is wokwi and the other is real hardware, so we use the same feature flag for pin configuration, kinda hacky
    #[cfg(feature = "epd5in83_v2")]
    let cs = PinDriver::output(peripherals.pins.gpio14)?;
    #[cfg(feature = "epd2in9_v2")]
    let cs = PinDriver::output(peripherals.pins.gpio5)?;

    let busy_in = PinDriver::input(peripherals.pins.gpio4)?;
    let dc = PinDriver::output(peripherals.pins.gpio22)?;
    let rst = PinDriver::output(peripherals.pins.gpio21)?;

    let config = Config::new().baudrate(112500.into());
    let mut device = SpiDeviceDriver::new_single(
        spi,
        sclk,
        serial_out,
        Option::<Gpio2>::None,
        Option::<AnyIOPin>::None,
        &SpiDriverConfig::default(),
        &config,
    )?;

    let mut delay = Ets;

    Delay::delay_ms(3000);
    let mut epd = Epd::new(&mut device, cs, busy_in, dc, rst, &mut delay, None)?;
    info!("E-Ink display init completed!");

    //Set up a channel to send messages received from the MQTT queue (separate thread) to the main thread, to display them on the e-paper module
    info!("Setting up the MQTT client...");
    let (sender, receiver) = mpsc::channel::<String>();
    let _mqtt_client: EspMqttClient<ConnState<MessageImpl, EspError>> = setup_mqtt_client(sender)?;

    loop {
        Delay::delay_ms(3000);
        // Check for new messages every 3 seconds for 2 seconds
        let message = receiver.recv_timeout(Duration::from_millis(2000));
        if let Ok(message) = message {
            info!("Message received in main thread: {:?}", message);
            let events: Vec<SEvent> = serde_json::from_str(message.as_str())?;
            // Dummy events for testing the display
            // let events: Vec<SEvent> = vec![
            //     SEvent {
            //         id: "1".to_string(),
            //         title: "Test 1: the test".to_string(),
            //         datetime: String::from("2022/05/20 11:00"),
            //         timestamp: 1688690076,
            //         building: "F3".to_string(),
            //         room: "P3".to_string(),
            //     },
            //     SEvent {
            //         id: "2".to_string(),
            //         title: "Test 2: the other test".to_string(),
            //         datetime: String::from("2022/05/20 11:00"),
            //         timestamp: 1688690076,
            //         building: "F3".to_string(),
            //         room: "P3".to_string(),
            //     },
            // ];

            display.clear(Color::White)?;
            let mut i = 0;
            let mut draw_room = true;
            for event in events {
                if draw_room {
                    draw_text(&mut display, format!(" {} ({}) ", event.room, event.building).as_str(), DISPLAY_CENTER, i, Alignment::Center);
                    i += 20;
                    draw_room = false;
                }
                draw_text(&mut display, format!(" {} ", &event.title).as_str(), 0, i, Alignment::Left);
                draw_text(&mut display, format!(" {} ", &event.datetime).as_str(), DISPLAY_END, i, Alignment::Right);
                i += FONT_HEIGHT;
            }
            epd.update_frame(&mut device, display.buffer(), &mut delay)?;
            epd.display_frame(&mut device, &mut delay)?;
        }
    }
}

fn configure_wifi(wifi: &mut BlockingWifi<EspWifi>) -> Result<(), EspError> {
    wifi.set_configuration(&Configuration::Client(ClientConfiguration {
        ssid: WIFI_SSID.into(),
        password: WIFI_PASS.into(),
        auth_method: AuthMethod::None,
        ..Default::default()
    }))?;
    wifi.start()?;
    info!("Wifi started!");

    wifi.connect()?;
    info!("Wifi connected!");

    wifi.wait_netif_up()?;
    info!("Wifi ready!");

    Ok(())
}

fn setup_mqtt_client(
    sender: Sender<String>,
) -> Result<EspMqttClient<ConnState<MessageImpl, EspError>>, EspError> {
    info!("About to start MQTT client");

    let mut conf = MqttClientConfiguration {
        client_id: Some(MQTT_CLIENT_ID),
        crt_bundle_attach: Some(esp_idf_sys::esp_crt_bundle_attach),
        ..Default::default()
    };
    #[cfg(feature = "load_certs")]
    {
        let server_cert_bytes: Vec<u8> =
            include_bytes!("../certificates/AmazonRootCA1.pem").to_vec();
        let client_cert_bytes: Vec<u8> =
            include_bytes!("../certificates/display.client.crt").to_vec();
        let private_key_bytes: Vec<u8> =
            include_bytes!("../certificates/display.private.key").to_vec();

        let server_cert: X509 = convert_certificate(server_cert_bytes);
        let client_cert: X509 = convert_certificate(client_cert_bytes);
        let private_key: X509 = convert_certificate(private_key_bytes);

        conf.server_certificate = Some(server_cert);
        conf.client_certificate = Some(client_cert);
        conf.private_key = Some(private_key);
    }

    let (mut client, mut connection) = EspMqttClient::new_with_conn(MQTT_ENDPOINT, &conf)?;

    info!("MQTT client started!");

    thread::spawn(move || {
        info!("MQTT Listening for messages...");

        // Send received messages back to the main thread to display them
        while let Some(msg) = connection.next() {
            match msg {
                Err(e) => info!("MQTT Message ERROR: {}", e),
                Ok(msg) => {
                    info!("MQTT Message: {:?}", msg);
                    if let Event::Received(msg) = msg {
                        let parsed_string = String::from_utf8(msg.data().to_vec());
                        if let Ok(parsed_string) = parsed_string {
                            info!("Parsed MQTT message: {:?}", parsed_string);
                            sender.send(parsed_string).unwrap();
                        }
                    }
                }
            }
        }

        info!("MQTT connection loop exit");
    });

    client.subscribe(MQTT_TOPIC_NAME, QoS::AtMostOnce)?;

    info!("Subscribed to all topics ({})", MQTT_TOPIC_NAME);

    // Delay::delay_ms(1000);
    // // This will be the first message appearing on the screen
    // client.publish(
    //     MQTT_TOPIC_NAME,
    //     QoS::AtMostOnce,
    //     false,
    //     format!("Hello from {}!", MQTT_TOPIC_NAME).as_bytes(),
    // )?;

    // info!(
    //     "Published a hello message to topic \"{}\".",
    //     MQTT_TOPIC_NAME
    // );

    Ok(client)
}

#[cfg(feature = "load_certs")]
fn convert_certificate(mut certificate_bytes: Vec<u8>) -> X509<'static> {
    // append NUL
    certificate_bytes.push(0);

    // convert the certificate
    let certificate_slice: &[u8] = unsafe {
        let ptr: *const u8 = certificate_bytes.as_ptr();
        let len: usize = certificate_bytes.len();
        mem::forget(certificate_bytes);

        slice::from_raw_parts(ptr, len)
    };

    // return the certificate file in the correct format
    X509::pem_until_nul(certificate_slice)
}

#[cfg(feature = "epd2in9_v2")]
pub fn draw_text(display: &mut EpdDisplay, text: &str, x: i32, y: i32, align: Alignment) {
    display.set_rotation(DisplayRotation::Rotate90);
    let style = MonoTextStyleBuilder::new()
        .font(&embedded_graphics::mono_font::ascii::FONT_7X13_BOLD)
        .text_color(Color::Black)
        .background_color(Color::White)
        .build();

    let text_style = TextStyleBuilder::new().baseline(Baseline::Top).alignment(align).build();

    let _ = Text::with_text_style(text, Point::new(x, y), style, text_style).draw(display);
}

#[cfg(feature = "epd5in83_v2")]
pub fn draw_text(display: &mut EpdDisplay, text: &str, x: i32, y: i32, align: Alignment) {
    let style = MonoTextStyleBuilder::new()
        .font(&embedded_graphics::mono_font::ascii::FONT_10X20)
        .text_color(Color::Black)
        .background_color(Color::White)
        .build();

    let text_style = TextStyleBuilder::new().baseline(Baseline::Top).alignment(align).build();

    let _ = Text::with_text_style(text, Point::new(x, y), style, text_style).draw(display);
}

#[derive(Deserialize)]
pub struct SEvent {
    pub id: String,
    pub title: String,
    pub timestamp: u64,
    pub datetime: String,
    pub building: String,
    pub room: String,
}
