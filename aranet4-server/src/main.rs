use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use btleplug::api::{Central, Manager as _, Peripheral as _, ScanFilter, WriteType};
use btleplug::platform::{Manager, Peripheral};
use byteorder::{LittleEndian, ReadBytesExt};
use serde::{Deserialize, Serialize};
use std::{io::Cursor, net::SocketAddr, sync::Arc, time::Duration};
use tokio::sync::RwLock;
use tracing::{error, info, warn};
use uuid::Uuid;

// Aranet4 Characteristic UUID for current readings
const CURRENT_READINGS_UUID: Uuid = uuid::uuid!("f0cd3001-95da-4f4b-9ac8-aa55d312af0c");

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SensorReading {
    co2: u16,
    temperature: f32,
    humidity: u8,
    pressure: u16,
    battery: u8,
    timestamp: u64,
    status: String,
}

#[derive(Clone)]
struct AppState {
    latest_reading: Arc<RwLock<Option<SensorReading>>>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    info!("Starting Aranet4 HTTP Server");

    let state = AppState {
        latest_reading: Arc::new(RwLock::new(None)),
    };

    // Spawn background task to read from Aranet4
    let bg_state = state.clone();
    tokio::spawn(async move {
        aranet_reader_task(bg_state).await;
    });

    // Build HTTP router
    let app = Router::new()
        .route("/", get(root))
        .route("/api/sensor", get(get_sensor_data))
        .route("/health", get(health))
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    info!("Listening on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

async fn aranet_reader_task(state: AppState) {
    loop {
        match read_aranet().await {
            Ok(reading) => {
                info!(
                    "✓ Read from Aranet4: CO2={} ppm, Temp={:.1}°C, Humidity={}%, Battery={}%",
                    reading.co2, reading.temperature, reading.humidity, reading.battery
                );
                *state.latest_reading.write().await = Some(reading);
            }
            Err(e) => {
                error!("Failed to read from Aranet4: {}", e);
            }
        }

        tokio::time::sleep(Duration::from_secs(30)).await;
    }
}

async fn read_aranet() -> anyhow::Result<SensorReading> {
    info!("Scanning for Aranet4 device...");

    let manager = Manager::new().await?;
    let adapters = manager.adapters().await?;

    if adapters.is_empty() {
        anyhow::bail!("No Bluetooth adapters found");
    }

    let adapter = &adapters[0];
    info!("Using Bluetooth adapter");

    // Start scanning
    adapter.start_scan(ScanFilter::default()).await?;

    // Give it time to find devices
    tokio::time::sleep(Duration::from_secs(5)).await;

    let peripherals = adapter.peripherals().await?;
    info!("Found {} BLE device(s)", peripherals.len());

    // Find Aranet4
    let mut aranet_device: Option<Peripheral> = None;
    for peripheral in peripherals {
        if let Ok(Some(properties)) = peripheral.properties().await {
            if let Some(name) = properties.local_name {
                info!("Found device: {}", name);
                if name.starts_with("Aranet4") {
                    info!("✓ Found Aranet4: {}", name);
                    aranet_device = Some(peripheral);
                    break;
                }
            }
        }
    }

    adapter.stop_scan().await?;

    let device = aranet_device.ok_or_else(|| anyhow::anyhow!("Aranet4 not found"))?;

    // Connect
    info!("Connecting to Aranet4...");
    if !device.is_connected().await? {
        device.connect().await?;
        info!("✓ Connected");
    }

    // Discover services
    info!("Discovering services...");
    device.discover_services().await?;

    // Find the current readings characteristic
    let chars = device.characteristics();
    let current_readings_char = chars
        .iter()
        .find(|c| c.uuid == CURRENT_READINGS_UUID)
        .ok_or_else(|| anyhow::anyhow!("Current readings characteristic not found"))?;

    info!("✓ Found current readings characteristic");

    info!("Reading sensor data...");
    let data = device.read(current_readings_char).await?;
    info!("✓ Read {} bytes", data.len());

    // Parse the data (same format as Arduino library)
    let mut cursor = Cursor::new(data);
    let co2 = cursor.read_u16::<LittleEndian>()?;
    let temperature_raw = cursor.read_u16::<LittleEndian>()?;
    let pressure_raw = cursor.read_u16::<LittleEndian>()?;
    let humidity = cursor.read_u8()?;
    let battery = cursor.read_u8()?;
    let status = cursor.read_u8()?;
    let _interval = cursor.read_u16::<LittleEndian>()?;
    let _ago = cursor.read_u16::<LittleEndian>()?;

    let temperature = temperature_raw as f32 / 20.0;
    let pressure = pressure_raw / 10;

    let status_text = match status {
        1 => "GREEN",
        2 => "YELLOW",
        3 => "RED",
        _ => "UNKNOWN",
    };

    // Disconnect
    device.disconnect().await?;
    info!("Disconnected");

    Ok(SensorReading {
        co2,
        temperature,
        humidity,
        pressure,
        battery,
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs(),
        status: status_text.to_string(),
    })
}

async fn root() -> &'static str {
    "Aranet4 HTTP Server\n\nEndpoints:\n  GET /api/sensor - Get current sensor data\n  GET /health - Health check\n"
}

async fn health() -> impl IntoResponse {
    StatusCode::OK
}

async fn get_sensor_data(State(state): State<AppState>) -> Response {
    match state.latest_reading.read().await.as_ref() {
        Some(reading) => Json(reading).into_response(),
        None => (
            StatusCode::SERVICE_UNAVAILABLE,
            "No sensor data available yet. Waiting for first reading...",
        )
            .into_response(),
    }
}
