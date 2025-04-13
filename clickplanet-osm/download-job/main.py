#!/usr/bin/env python3
"""
OSM Data HTTP to GCS Transfer Tool

This script transfers OpenStreetMap data from HTTP source to Google Cloud Storage.
It handles large files (70+ GB) by splitting them into manageable chunks that are
processed in parallel, using GCS multipart upload and composition for efficiency.
"""

import os
import sys
import time
import threading
import traceback
import concurrent.futures
import psutil
import requests

# Import modules
from utils import (
    logger, DEBUG, MB, GB
)
from download import (
    get_remote_file_size, get_osm_config, download_chunk_to_memory
)
from upload import upload_file_part_to_gcs, compose_parts


def transfer_osm_data_to_gcs(extract_name):
    """Transfer OSM data from HTTP source to GCS using parallel multipart uploads
    
    Args:
        extract_name: Name of the OSM extract to process (e.g., 'france', 'planet')
    """
    start_time_total = time.time()
    config = None
    
    try:
        # === INITIALIZATION ===
        logger.info(f"Starting OSM download process for extract: {extract_name}")
        
        # Get configuration for the specified extract
        osm_config = get_osm_config(extract_name)
        if not osm_config:
            sys.exit(1)  # Error already logged in get_osm_config
            
        url = osm_config['url']
        filename = osm_config['filename']
        logger.info(f"Using source URL: {url}")
        
        # Verify GCS bucket information
        bucket_name = os.environ.get('BUCKET_NAME')
        if not bucket_name:
            logger.error("Error: BUCKET_NAME environment variable is required")
            sys.exit(1)
        
        logger.info(f"Using GCS bucket: {bucket_name}")
        
        # Define GCS storage locations
        final_object_name = f"osm_data/{filename}"
        logger.info(f"Parts will be uploaded as temporary objects")
        logger.info(f"Final composed file will be: gs://{bucket_name}/{final_object_name}")
        
        # Check for available disk space
        if DEBUG:
            try:
                disk_usage = psutil.disk_usage('/tmp')
                logger.debug(f"Available disk space in /tmp: {disk_usage.free/GB:.2f} GB ({disk_usage.percent}% used)")
            except ImportError:
                logger.debug("psutil not available, skipping disk space check")
        
        config = {
            'url': url,
            'filename': filename,
            'bucket_name': bucket_name,
            'final_object_name': final_object_name
        }
        
        logger.info(f'Starting download of {extract_name} OSM data')
        
        # === CHUNK CALCULATION ===
        # Get the total size of the file
        total_size = get_remote_file_size(url)
        if not total_size:
            logger.error(f"Error: Could not determine file size for {url}")
            sys.exit(1)
        
        logger.info(f"Total file size: {total_size / GB:.2f} GB")
        
        # Calculate chunks - use 512 MB chunks
        chunk_size = 256 * MB
        num_chunks = (total_size + chunk_size - 1) // chunk_size
        logger.info(f"Splitting download into {num_chunks} chunks of {chunk_size / MB:.0f} MB each")
        
        downloaded_bytes = 0
        lock = threading.Lock()
        uploaded_chunks = []
        max_workers = min(4, os.cpu_count() or 4)
        
        def update_progress(bytes_downloaded):
            nonlocal downloaded_bytes
            with lock:
                downloaded_bytes += bytes_downloaded
                # Silently track progress without printing status lines
        
        logger.info(f"Processing chunks in parallel with {max_workers} workers")
        
        def process_chunk(i):
            chunk_start_time = time.time()
            
            start_byte = i * chunk_size
            end_byte = min((i + 1) * chunk_size - 1, total_size - 1)
            actual_chunk_size = end_byte - start_byte + 1
            
            # No need to define explicit chunk file names - upload_file_part_to_gcs handles it
            
            with lock:
                logger.info(f"\nProcessing chunk {i+1} of {num_chunks} ({i/num_chunks*100:.1f}% complete)")
                logger.info(f"Chunk size: {actual_chunk_size/MB:.2f} MB")
            
            if DEBUG:
                try:
                    process = psutil.Process(os.getpid())
                    memory_info = process.memory_info()
                    with lock:
                        logger.debug(f"Memory usage before download: RSS={memory_info.rss/MB:.2f} MB, VMS={memory_info.vms/MB:.2f} MB")
                except ImportError:
                    with lock:
                        logger.debug("psutil not available, skipping memory usage check")
            
            try:
                process_start = time.time()
                
                with lock:
                    logger.info(f"Downloading chunk {i+1} from source (bytes {start_byte}-{end_byte})")
                
                try:
                    # Use the proper in-memory download function
                    success, result = download_chunk_to_memory(
                        url,
                        start_byte,
                        end_byte,
                        progress_callback=update_progress
                    )
                    
                    if not success:
                        with lock:
                            logger.error(f"Error downloading chunk {i+1}: {result}")
                        return (False, result)
                        
                    # Get the data from the result
                    chunk_data = result
                    
                except Exception as e:
                    with lock:
                        logger.error(f"Error downloading chunk {i+1}: {str(e)}")
                    return (False, str(e))
                
                with lock:
                    logger.info(f"Downloaded chunk {i+1} ({len(chunk_data)/MB:.2f} MB)")
                    logger.info(f"Uploading chunk {i+1} as part to GCS")
                
                part_blob = upload_file_part_to_gcs(
                    bucket_name, 
                    final_object_name,  # This is the base name, part suffix will be added by function
                    chunk_data, 
                    i,  # Part number
                    content_type='application/octet-stream'
                )
                
                process_time = time.time() - process_start
                
                with lock:
                    uploaded_chunks.append(part_blob)
                    logger.info(f"Successfully uploaded chunk {i+1} as '{part_blob.name}' to GCS in {process_time:.2f} seconds")
                
                chunk_time = time.time() - chunk_start_time
                with lock:
                    logger.info(f"Completed chunk {i+1} in {chunk_time:.2f} seconds")
                
                return (True, part_blob)
                
            except Exception as e:
                with lock:
                    logger.error(f"\nError processing chunk {i+1}: {e}")
                    if DEBUG:
                        logger.error(traceback.format_exc())
                return (False, str(e))
        
        with concurrent.futures.ThreadPoolExecutor(max_workers=max_workers) as executor:
            futures = [executor.submit(process_chunk, i) for i in range(num_chunks)]
            
            for i, future in enumerate(concurrent.futures.as_completed(futures)):
                success, result = future.result()
                if not success:
                    logger.error(f"Failed to process chunk: {result}")
                    sys.exit(1)
            
            logger.info(f"All chunks uploaded successfully. Composing final object...")
            try:
                # Log all part names before composition
                logger.info("Composing the following parts:")
                for i, blob in enumerate(uploaded_chunks):
                    logger.info(f"  Part {i+1}: {blob.name}")
                    
                composed_blob = compose_parts(
                    bucket_name,
                    final_object_name,
                    uploaded_chunks,
                    delete_parts=True,  # Clean up part objects after composition
                    content_type='application/octet-stream'
                )
                logger.info(f"Successfully composed final object: gs://{bucket_name}/{composed_blob.name}")
                logger.info(f"Final object size: {composed_blob.size/MB:.2f} MB")
            except Exception as e:
                logger.error(f"Error composing parts: {e}")
                if DEBUG:
                    logger.error(traceback.format_exc())
                sys.exit(1)
        
        total_time = time.time() - start_time_total
        hours, remainder = divmod(total_time, 3600)
        minutes, seconds = divmod(remainder, 60)
        
        logger.info(f"\nAll {num_chunks} chunks successfully processed in {hours:.0f}h {minutes:.0f}m {seconds:.0f}s")
        logger.info(f"Average processing time per chunk: {total_time/num_chunks:.2f} seconds")
        logger.info(f"Data available at: gs://{bucket_name}/{final_object_name}")
        
    except Exception as e:
        logger.error(f"Error during download/upload process: {e}")
        if DEBUG:
            logger.error(traceback.format_exc())
        sys.exit(1)
    finally:
        # No temporary directories to clean up
        pass

# ===== Entry Point =====

def main():
    """Main entry point"""
    extract = os.environ.get('EXTRACT', 'france')
    transfer_osm_data_to_gcs(extract)

if __name__ == '__main__':
    main()
