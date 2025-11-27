-- Create sensor_readings table
CREATE TABLE IF NOT EXISTS sensor_readings (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    timestamp INTEGER NOT NULL,
    co2 INTEGER NOT NULL,
    temperature REAL NOT NULL,
    humidity INTEGER NOT NULL,
    pressure INTEGER NOT NULL,
    battery INTEGER NOT NULL,
    status TEXT NOT NULL
);

-- Create index on timestamp for efficient time-based queries
CREATE INDEX IF NOT EXISTS idx_timestamp ON sensor_readings(timestamp);

-- Create index on co2 for quick lookups of CO2 levels
CREATE INDEX IF NOT EXISTS idx_co2 ON sensor_readings(co2);
