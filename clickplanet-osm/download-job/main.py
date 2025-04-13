import os
import sys
import time
import subprocess
from pathlib import Path
import aria2p
from google.cloud import storage

def ensure_aria2_daemon():
    """Start aria2c daemon if not running"""
    try:
        # Try to connect to existing daemon
        aria2 = aria2p.API(aria2p.Client())
        aria2.get_version()
        return aria2
    except Exception:
        # Start new daemon
        subprocess.Popen(
            ["aria2c", "--enable-rpc", "--rpc-listen-all=false"],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL
        )
        # Wait for daemon to start
        time.sleep(2)
        return aria2p.API(aria2p.Client())

def upload_to_gcs(file_path, bucket, blob_name):
    """Upload a file to GCS using the storage client"""
    blob = bucket.blob(blob_name)
    
    print(f"Starting upload of {file_path} to {blob_name}")
    blob.upload_from_filename(
        file_path,
        content_type='application/octet-stream',
        timeout=3600,  # 1 hour timeout
        retry=storage.retry.Retry(deadline=3600)  # Retry for up to 1 hour
    )
    print(f"Upload of {blob_name} completed")

def download_osm(extract):
    # Configure URLs and paths
    if extract == 'france':
        url = 'https://download.geofabrik.de/europe/france-latest.osm.pbf'
        filename = 'france-latest.osm.pbf'
    elif extract == 'planet':
        url = 'https://planet.openstreetmap.org/pbf/planet-latest.osm.pbf'
        filename = 'planet-latest.osm.pbf'
    else:
        print(f'Error: Invalid extract {extract}')
        sys.exit(1)

    bucket_name = os.environ.get('BUCKET_NAME')
    if not bucket_name:
        print('Error: BUCKET_NAME environment variable is required')
        sys.exit(1)
    
    download_dir = os.environ.get('DOWNLOAD_DIR', '/downloads')
    
    try:
        print(f'Starting download of {extract} OSM data from {url}')
        
        # Initialize aria2 daemon and API
        aria2 = ensure_aria2_daemon()
        
        # Ensure download directory exists
        download_dir = Path(download_dir)
        download_dir.mkdir(parents=True, exist_ok=True)
        
        # Configure download options
        options = {
            "dir": str(download_dir),  # Use configured directory
            "out": filename,
            "split": "8",  # Number of connections per download
            "max-connection-per-server": "8",
            "min-split-size": "20M",  # Minimum split size for parallel download
            "continue": "true",  # Resume partially downloaded files
            "max-tries": "5",    # Retry on failure
            "retry-wait": "10",  # Wait between retries
            "console-log-level": "notice"
        }
        
        # Start download
        download = aria2.add_uris([url], options=options)
        
        # Monitor download progress
        while not download.is_complete:
            download.update()
            progress = download.progress
            speed = download.download_speed / (1024*1024)  # Convert to MB/s
            print(f'Progress: {progress:.1f}% Speed: {speed:.2f} MB/s', end='\r')
            time.sleep(1)
        
        print('\nDownload completed successfully')
        
        # Initialize GCS client
        storage_client = storage.Client()
        bucket = storage_client.bucket(bucket_name)
        
        # Upload file to GCS
        print(f'Starting upload to gs://{bucket_name}/raw/{filename}')
        file_path = str(download_dir / filename)
        upload_to_gcs(file_path, bucket, f'raw/{filename}')
        print('Upload completed successfully')
        
        # Clean up
        os.remove(file_path)
        print('Local file cleaned up')
        
        print(f'File available at: gs://{bucket_name}/raw/{filename}')
        
    except Exception as e:
        print(f'Error: {str(e)}')
        sys.exit(1)

if __name__ == '__main__':
    extract = os.environ.get('EXTRACT', 'france')
    download_osm(extract)
