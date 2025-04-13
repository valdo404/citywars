import os
import sys
import requests
from google.cloud import storage

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
    
    try:
        print(f'Starting download of {extract} OSM data from {url}')
        
        # Initialize GCS client
        storage_client = storage.Client()
        
        # Start the download with streaming
        with requests.get(url, stream=True) as response:
            response.raise_for_status()
            
            # Get total size if available
            total_size = int(response.headers.get('content-length', 0))
            print(f'Total download size: {total_size / (1024*1024*1024):.2f} GB')
            
            # Set up GCS upload
            object_name = f"raw/{filename}"
            bucket = storage_client.bucket(bucket_name)
            blob = bucket.blob(object_name)
            
            try:
                # Start the upload
                print(f'Starting upload to gs://{bucket_name}/{object_name}')
                
                # Upload the file with progress tracking
                uploaded = 0
                
                def callback(progress):
                    nonlocal uploaded
                    uploaded += progress.bytes_sent
                    if total_size:
                        print(f'Progress: {uploaded/total_size*100:.1f}% ({uploaded/(1024*1024*1024):.2f} GB / {total_size/(1024*1024*1024):.2f} GB)')
                
                blob.upload_from_file(
                    response.raw,
                    content_type='application/octet-stream',
                    size=total_size,
                    timeout=3600,  # 1 hour timeout
                    checksum=None,  # Skip checksum validation
                    rewind=False,  # Don't rewind since we're streaming
                    if_generation_match=None  # Allow overwrite
                )
                
            except Exception as e:
                print(f'Error during upload: {str(e)}')
                raise
            
        print('Download and upload completed successfully')
        
    except Exception as e:
        print(f'Error: {str(e)}')
        sys.exit(1)

if __name__ == '__main__':
    extract = os.environ.get('EXTRACT', 'france')
    download_osm(extract)
