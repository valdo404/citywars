-- Create the cities table for OSM data with enhanced fields for the virtual democracy system
CREATE TABLE IF NOT EXISTS cities (
  city_id BIGINT PRIMARY KEY,
  name VARCHAR(255),
  lat DOUBLE PRECISION,
  lon DOUBLE PRECISION,
  population BIGINT,
  place_type VARCHAR(50),     -- Type of settlement (city, town, village, etc.)
  admin_level VARCHAR(10),    -- Administrative level in hierarchy
  capital VARCHAR(50),        -- Whether it's a capital and what type
  country_code VARCHAR(10),   -- Country code for internationalization
  tags_json TEXT,             -- All OSM tags in JSON format
  wkt TEXT                    -- Well-known text representation for spatial functions
);

-- Create the roads table for OSM data
CREATE TABLE IF NOT EXISTS roads (
  road_id BIGINT PRIMARY KEY,
  name VARCHAR(255),
  nodes_string TEXT,
  tags_json TEXT,
  wkt TEXT
);
