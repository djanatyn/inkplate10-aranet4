# inkplate10-aranet4

rust server querying aranet4, with thin display client on inkplate10

<p align="center">
  <img src="https://github.com/djanatyn/inkplate10-aranet4/raw/main/aranet4.jpg" alt="aranet4 device next to inkplate10 display">
</p>

## setup (rust server)

### identifying your service ID

``` rust
❯ cargo run --example scanner
   Compiling aranet4-server v0.1.0 (/Users/jstrickland/code/inkplate10-aranet4/aranet4-server)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.78s
     Running `target/debug/examples/scanner`
Starting BLE scanner...

✓ Found 1 Bluetooth adapter(s)
Starting scan for 10 seconds...

Found 7 device(s):

...

Device 2:
  Name: Aranet4 1BE74
  Address: 00:00:00:00:00:00
  RSSI: Some(-66)
  Services: [f0cd1400-95da-4f4b-9ac8-aa55d312af0c] 
```

update the service uuid in `aranet4-server/src/main.rs`:

``` rust
// Aranet4 Characteristic UUID for current readings
const CURRENT_READINGS_UUID: Uuid = uuid::uuid!("f0cd3001-95da-4f4b-9ac8-aa55d312af0c");
```

### running the server

```bash
# run server
cargo run
```

```
2025-11-26T02:54:43.848709Z  INFO aranet4_server: Starting Aranet4 HTTP Server
2025-11-26T02:54:43.849801Z  INFO aranet4_server: Scanning for Aranet4 device...
2025-11-26T02:54:43.850362Z  INFO aranet4_server: Listening on http://0.0.0.0:3000
2025-11-26T02:54:43.865428Z  INFO aranet4_server: Using Bluetooth adapter
2025-11-26T02:54:48.866877Z  INFO aranet4_server: Found 6 BLE device(s)
2025-11-26T02:54:48.867007Z  INFO aranet4_server: Found device: Aranet4 1BE74
2025-11-26T02:54:48.867026Z  INFO aranet4_server: ✓ Found Aranet4: Aranet4 1BE74
2025-11-26T02:54:48.867068Z  INFO aranet4_server: Connecting to Aranet4...
2025-11-26T02:54:51.074382Z  INFO aranet4_server: ✓ Connected
2025-11-26T02:54:51.074482Z  INFO aranet4_server: Discovering services...
2025-11-26T02:54:51.074588Z  INFO aranet4_server: ✓ Found current readings characteristic
2025-11-26T02:54:51.074627Z  INFO aranet4_server: Reading sensor data...
2025-11-26T02:54:51.133948Z  INFO aranet4_server: ✓ Read 13 bytes
2025-11-26T02:54:51.135489Z  INFO aranet4_server: Disconnected
2025-11-26T02:54:51.135674Z  INFO aranet4_server: ✓ Read from Aranet4: CO2=602 ppm, Temp=21.3°C, Humidity=52%, Battery=73%
```

### communicating with the server

#### `/`

```bash
; curl localhost:3000
``` 

```
Aranet4 HTTP Server

Endpoints:
  GET /api/sensor - Get current sensor data
  GET /health - Health check
```

#### `/health`

```
; curl localhost:3000/health -vL
``` 

```
* Host localhost:3000 was resolved.
* IPv6: ::1
* IPv4: 127.0.0.1
*   Trying [::1]:3000...
* connect to ::1 port 3000 from ::1 port 64681 failed: Connection refused
*   Trying 127.0.0.1:3000...
* Connected to localhost (127.0.0.1) port 3000
> GET /health HTTP/1.1
> Host: localhost:3000
> User-Agent: curl/8.7.1
> Accept: */*
>
* Request completely sent off
< HTTP/1.1 200 OK
< content-length: 0
< date: Wed, 26 Nov 2025 02:55:33 GMT
<
* Connection #0 to host localhost left intact
```

#### `/api/sensor`

```
❯ curl localhost:3000/api/sensor
```

```json
{
  "co2": 555,
  "temperature": 21.3,
  "humidity": 52,
  "pressure": 974,
  "battery": 73,
  "timestamp": 1764125728,
  "status": "GREEN"
}
```
   
## setup (arduino thin client)

```bash
# install dependencies with arduino-cli
arduino-cli lib install ArduinoJson

# setup configuration file
cd Aranet4_Thin_Client
cp config.h.example config.h
vim config.h  # edit with your WiFi credentials and server IP

# compile and upload (from project root)
cd ..
arduino-cli compile --upload \
  -p /dev/cu.usbserial-XXXXX \
  --fqbn Inkplate_Boards:esp32:Inkplate10:UploadSpeed=115200 \
  Aranet4_Thin_Client

# monitor on serial after successful upload
arduino-cli monitor -p /dev/cu.usbserial-XXXXX -c baudrate=115200
```

---

## built with

- https://github.com/m1guelpf/aranet-rs
- https://github.com/SolderedElectronics/Inkplate-Arduino-library
- https://lib.rs/crates/axum
