use btleplug::api::{Central, Manager as _, Peripheral as _, ScanFilter};
use btleplug::platform::Manager;
use std::time::Duration;
use tokio::time;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Starting BLE scanner...\n");

    let manager = Manager::new().await?;
    let adapters = manager.adapters().await?;

    if adapters.is_empty() {
        println!("❌ No Bluetooth adapters found!");
        return Ok(());
    }

    println!("✓ Found {} Bluetooth adapter(s)", adapters.len());
    let adapter = &adapters[0];

    println!("Starting scan for 10 seconds...\n");
    adapter.start_scan(ScanFilter::default()).await?;

    time::sleep(Duration::from_secs(10)).await;

    let peripherals = adapter.peripherals().await?;

    println!("Found {} device(s):\n", peripherals.len());

    for (i, peripheral) in peripherals.iter().enumerate() {
        if let Ok(Some(properties)) = peripheral.properties().await {
            println!("Device {}:", i + 1);
            if let Some(name) = properties.local_name {
                println!("  Name: {}", name);
            } else {
                println!("  Name: <unnamed>");
            }
            println!("  Address: {:?}", properties.address);
            println!("  RSSI: {:?}", properties.rssi);
            if !properties.services.is_empty() {
                println!("  Services: {:?}", properties.services);
            }
            println!();
        }
    }

    adapter.stop_scan().await?;
    Ok(())
}
