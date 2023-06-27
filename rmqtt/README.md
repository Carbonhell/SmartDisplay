# Prerequisites
Both the simulated and the real hardware require the xtensa rustup toolchain to be available on the system. You can follow the instructions [here](https://esp-rs.github.io/book/installation/index.html#risc-v-and-xtensa-targets) on how to do so easily with espup. **Be sure to also follow the std Development Requirements section.**

To simulate the hardware on Wokwi, you'll need the [VSCode extension](https://docs.wokwi.com/vscode/getting-started).

You will also need [cargo espflash](https://github.com/esp-rs/espflash/blob/main/cargo-espflash/README.md#installation) if you plan to run the code on real hardware.

## Configuration
In the main.rs file, you will find several consts that need to have the correct values for the code to work.
-) WIFI_SSID & WIFI_PASS: for Wokwi, they're already set. If you want to use real hardware, you'll need to put the credentials of your access point.
-) MQTT_ENDPOINT: change this to the ATS endpoint of your IoT Core AWS profile. It should also work with other MQTT providers, such as [EMQX](https://www.emqx.com/en/mqtt/public-mqtt5-broker).
-) MQTT_CLIENT_ID: This should be the thing's name if you're using AWS IoT core.
-) MQTT_TOPIC_NAME: Be sure to use a topic you have access to (check the policy attached to the certificare you're using)

You also need to put your identity certificates in the `certificates` folder, with the correct file names, along with the Amazon Root CA:
`-) `AmazonRootCA1.pem` (you can download this when you create a certificate manually)
-) `display.client.crt`
-) `display.private.key``

## Simulated
For the simulated hardware, you'll need the Wokwi VSCode extension (and therefore VSCode as well).
Once done, build your code with:
```sh
cargo build --features="epd2in9_v2,load_certs"
```
After this, you can start the Wokwi simulator.

And then start wok

## Real hardware
```sh
 cargo espflash --release --monitor --partition-table partition-table.csv --features="epd5in83_v2,load_certs"
```

---
# ESP32 w/ 5.83" E-Ink Waveshare display

## Configuration
1) Register your thing on AWS IoT with the correct policy and download the required certificates, along with the AWS root CA certificate.
2) Place them in the certificates folder, ensuring the filenames match with the AWS IoT certificate paths inside main.rs.
3) Set your AWS IoT MQTT endpoint in main.rs (MQTT_ENDPOINT).
4) Configure your WiFi credentials in main.rs.

## Flash
See https://esp-rs.github.io/book/tooling/espflash.html for details
```sh
cargo espflash --release --monitor --partition-table partition-table.csv
```

# TODOs:
1) Clean up the different feature requirements in separate files instead of having everything in main.rs
2) Test MQTT connection without certificates