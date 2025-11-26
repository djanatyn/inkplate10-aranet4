/*
   Aranet4 Thin Client for Inkplate10

   Connects to a local HTTP server serving Aranet4 sensor data
   and displays it on the Inkplate10 e-paper display.

   Setup:
   1. Copy config.h.example to config.h
   2. Edit config.h with your WiFi and server settings
   3. Upload to Inkplate10

   Note: config.h is gitignored and contains your credentials
*/

#include "Inkplate.h"
#include <WiFi.h>
#include <HTTPClient.h>
#include <ArduinoJson.h>
#include "config.h"

// API endpoint
const char* API_ENDPOINT = "/api/sensor";

// Update interval
const unsigned long UPDATE_INTERVAL = 30000; // 30 seconds

// Create Inkplate object in grayscale mode
Inkplate display(INKPLATE_3BIT);

// Timing
unsigned long lastUpdate = 0;

// Sensor data
struct SensorData {
    int co2;
    float temperature;
    int humidity;
    int pressure;
    int battery;
    String status;
    bool valid;
};

SensorData currentData = {0, 0.0, 0, 0, 0, "", false};

void setup() {
    Serial.begin(115200);
    Serial.println("Aranet4 Thin Client Starting...");

    // Initialize display
    display.begin();
    display.clearDisplay();
    display.setTextColor(0);
    display.setTextSize(2);

    // Show connecting message
    display.setCursor(300, 400);
    display.print("Connecting to WiFi...");
    display.display();

    // Connect to WiFi
    connectWiFi();

    // Initial data fetch
    fetchAndDisplay();
}

void loop() {
    // Check WiFi connection
    if (WiFi.status() != WL_CONNECTED) {
        Serial.println("WiFi disconnected, reconnecting...");
        connectWiFi();
    }

    // Periodic update
    if (millis() - lastUpdate >= UPDATE_INTERVAL) {
        fetchAndDisplay();
    }

    delay(1000);
}

void connectWiFi() {
    Serial.print("Connecting to WiFi: ");
    Serial.println(WIFI_SSID);

    WiFi.begin(WIFI_SSID, WIFI_PASSWORD);

    int attempts = 0;
    while (WiFi.status() != WL_CONNECTED && attempts < 30) {
        delay(500);
        Serial.print(".");
        attempts++;
    }

    if (WiFi.status() == WL_CONNECTED) {
        Serial.println("\nWiFi connected!");
        Serial.print("IP address: ");
        Serial.println(WiFi.localIP());
    } else {
        Serial.println("\nWiFi connection failed!");
    }
}

void fetchAndDisplay() {
    Serial.println("Fetching sensor data...");

    if (fetchSensorData()) {
        displayData();
    } else {
        displayError();
    }

    lastUpdate = millis();
}

bool fetchSensorData() {
    if (WiFi.status() != WL_CONNECTED) {
        Serial.println("WiFi not connected");
        return false;
    }

    HTTPClient http;
    String url = String("http://") + SERVER_HOST + ":" + SERVER_PORT + API_ENDPOINT;

    Serial.print("Requesting: ");
    Serial.println(url);

    http.begin(url);
    http.setTimeout(5000);

    int httpCode = http.GET();

    if (httpCode == HTTP_CODE_OK) {
        String payload = http.getString();
        Serial.println("Response received:");
        Serial.println(payload);

        // Parse JSON
        JsonDocument doc;
        DeserializationError error = deserializeJson(doc, payload);

        if (error) {
            Serial.print("JSON parse failed: ");
            Serial.println(error.c_str());
            http.end();
            return false;
        }

        // Extract data
        currentData.co2 = doc["co2"];
        currentData.temperature = doc["temperature"];
        currentData.humidity = doc["humidity"];
        currentData.pressure = doc["pressure"];
        currentData.battery = doc["battery"];
        currentData.status = doc["status"].as<String>();
        currentData.valid = true;

        http.end();
        return true;
    } else {
        Serial.print("HTTP request failed, code: ");
        Serial.println(httpCode);
        http.end();
        return false;
    }
}

void displayData() {
    display.clearDisplay();

    // Header
    display.setTextSize(3);
    display.setCursor(300, 30);
    display.print("Aranet4 Monitor");

    // Draw separator line
    display.drawLine(50, 80, 1150, 80, 0);

    int yPos = 140;
    int labelX = 100;
    int valueX = 600;
    int spacing = 100;

    display.setTextSize(3);

    // CO2 Level
    display.setCursor(labelX, yPos);
    display.print("CO2:");
    display.setCursor(valueX, yPos);
    display.print(currentData.co2);
    display.print(" ppm");

    // Add CO2 quality indicator
    String quality = getCO2Quality(currentData.co2);
    display.setTextSize(2);
    display.setCursor(valueX + 250, yPos + 10);
    display.print("(");
    display.print(quality);
    display.print(")");
    display.setTextSize(3);

    yPos += spacing + 20;

    // Temperature
    display.setCursor(labelX, yPos);
    display.print("Temperature:");
    display.setCursor(valueX, yPos);
    display.print(currentData.temperature, 1);
    display.print(" C");

    yPos += spacing;

    // Humidity
    display.setCursor(labelX, yPos);
    display.print("Humidity:");
    display.setCursor(valueX, yPos);
    display.print(currentData.humidity);
    display.print(" %");

    yPos += spacing;

    // Pressure
    display.setCursor(labelX, yPos);
    display.print("Pressure:");
    display.setCursor(valueX, yPos);
    display.print(currentData.pressure);
    display.print(" hPa");

    yPos += spacing;

    // Battery
    display.setCursor(labelX, yPos);
    display.print("Battery:");
    display.setCursor(valueX, yPos);
    display.print(currentData.battery);
    display.print(" %");

    // Draw battery icon
    drawBatteryIcon(900, yPos - 5, currentData.battery);

    // Footer
    display.setTextSize(2);
    display.setCursor(100, 750);
    display.print("WiFi: ");
    display.print(WiFi.localIP());

    display.setCursor(100, 780);
    display.print("Next update in ");
    unsigned long elapsed = millis() - lastUpdate;
    long remaining = (long)UPDATE_INTERVAL - (long)elapsed;
    if (remaining < 0) remaining = 0;
    display.print(remaining / 1000);
    display.print(" seconds");

    display.display();
}

void displayError() {
    display.clearDisplay();

    display.setTextSize(3);
    display.setCursor(300, 50);
    display.print("Aranet4 Monitor");

    display.setTextSize(2);
    display.setCursor(200, 300);
    display.print("Unable to fetch sensor data");

    display.setCursor(250, 350);
    display.print("Check server connection");

    display.setCursor(100, 450);
    display.print("Server: ");
    display.print(SERVER_HOST);
    display.print(":");
    display.print(SERVER_PORT);

    display.setCursor(100, 500);
    display.print("WiFi: ");
    if (WiFi.status() == WL_CONNECTED) {
        display.print("Connected (");
        display.print(WiFi.localIP());
        display.print(")");
    } else {
        display.print("Disconnected");
    }

    display.display();
}

String getCO2Quality(int co2) {
    if (co2 < 600) return "Excellent";
    if (co2 < 800) return "Good";
    if (co2 < 1000) return "Fair";
    if (co2 < 1400) return "Poor";
    return "Bad";
}

void drawBatteryIcon(int x, int y, int level) {
    int width = 60;
    int height = 30;

    // Battery outline
    display.drawRect(x, y, width, height, 0);

    // Battery terminal
    display.fillRect(x + width, y + 8, 6, 14, 0);

    // Fill level
    int fillWidth = (width - 4) * level / 100;
    if (fillWidth > 0) {
        display.fillRect(x + 2, y + 2, fillWidth, height - 4, 0);
    }
}
