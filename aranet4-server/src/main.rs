use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{Html, IntoResponse, Response},
    routing::get,
    Json, Router,
};
use btleplug::api::{Central, Manager as _, Peripheral as _, ScanFilter};
use btleplug::platform::{Manager, Peripheral};
use byteorder::{LittleEndian, ReadBytesExt};
use serde::{Deserialize, Serialize};
use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};
use sqlx::migrate::MigrateDatabase;
use sqlx::Row;
use std::{io::Cursor, net::SocketAddr, sync::Arc, time::Duration};
use tokio::sync::RwLock;
use tracing::{error, info};
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
    db: SqlitePool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    info!("Starting Aranet4 HTTP Server");

    // Initialize database
    let database_url = "sqlite://aranet4.db";

    // Create database if it doesn't exist
    if !sqlx::Sqlite::database_exists(database_url).await? {
        info!("Creating database...");
        sqlx::Sqlite::create_database(database_url).await?;
        info!("✓ Database created");
    }

    // Connect to database
    info!("Connecting to database...");
    let db = SqlitePoolOptions::new()
        .max_connections(5)
        .connect(database_url)
        .await?;
    info!("✓ Connected to database");

    // Run migrations
    info!("Running migrations...");
    sqlx::migrate!("./migrations")
        .run(&db)
        .await?;
    info!("✓ Migrations complete");

    let state = AppState {
        latest_reading: Arc::new(RwLock::new(None)),
        db,
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
        .route("/api/history", get(get_history_handler))
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

                // Store in database
                match store_reading(&state.db, &reading).await {
                    Ok(_) => info!("✓ Stored reading in database"),
                    Err(e) => error!("Failed to store reading in database: {}", e),
                }

                *state.latest_reading.write().await = Some(reading);
            }
            Err(e) => {
                error!("Failed to read from Aranet4: {}", e);
            }
        }

        tokio::time::sleep(Duration::from_secs(30)).await;
    }
}

async fn store_reading(db: &SqlitePool, reading: &SensorReading) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        INSERT INTO sensor_readings (timestamp, co2, temperature, humidity, pressure, battery, status)
        VALUES (?, ?, ?, ?, ?, ?, ?)
        "#
    )
    .bind(reading.timestamp as i64)
    .bind(reading.co2 as i64)
    .bind(reading.temperature)
    .bind(reading.humidity as i64)
    .bind(reading.pressure as i64)
    .bind(reading.battery as i64)
    .bind(&reading.status)
    .execute(db)
    .await?;

    Ok(())
}

async fn get_history(db: &SqlitePool, hours: Option<i64>, limit: i64) -> anyhow::Result<Vec<SensorReading>> {
    let rows = if let Some(hours) = hours {
        // Calculate timestamp threshold (current time - hours)
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs() as i64;
        let threshold = now - (hours * 3600);

        sqlx::query(
            r#"
            SELECT timestamp, co2, temperature, humidity, pressure, battery, status
            FROM sensor_readings
            WHERE timestamp >= ?
            ORDER BY timestamp ASC
            LIMIT ?
            "#
        )
        .bind(threshold)
        .bind(limit)
        .fetch_all(db)
        .await?
    } else {
        sqlx::query(
            r#"
            SELECT timestamp, co2, temperature, humidity, pressure, battery, status
            FROM sensor_readings
            ORDER BY timestamp DESC
            LIMIT ?
            "#
        )
        .bind(limit)
        .fetch_all(db)
        .await?
    };

    let mut readings = Vec::new();
    for row in rows {
        readings.push(SensorReading {
            timestamp: row.get::<i64, _>("timestamp") as u64,
            co2: row.get::<i64, _>("co2") as u16,
            temperature: row.get::<f32, _>("temperature"),
            humidity: row.get::<i64, _>("humidity") as u8,
            pressure: row.get::<i64, _>("pressure") as u16,
            battery: row.get::<i64, _>("battery") as u8,
            status: row.get::<String, _>("status"),
        });
    }

    // If using limit-based query, reverse to get chronological order
    if hours.is_none() {
        readings.reverse();
    }

    Ok(readings)
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

async fn root() -> Html<&'static str> {
    Html(include_str!("../static/index.html"))
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

#[derive(Deserialize)]
struct HistoryQuery {
    #[serde(default)]
    hours: Option<i64>,
    #[serde(default = "default_limit")]
    limit: i64,
}

fn default_limit() -> i64 {
    10000
}

async fn get_history_handler(
    State(state): State<AppState>,
    Query(params): Query<HistoryQuery>,
) -> Response {
    match get_history(&state.db, params.hours, params.limit).await {
        Ok(readings) => Json(readings).into_response(),
        Err(e) => {
            error!("Failed to fetch history: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to fetch sensor history",
            )
                .into_response()
        }
    }
}
