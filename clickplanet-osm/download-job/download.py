import os
import time
import traceback
import subprocess
import shutil

import requests

from utils import logger, DEBUG, KB, MB, GB

def cleanup_download_resources():
    logger.debug("No cleanup needed for in-memory downloads")

def get_remote_file_size(url):
    try:
        logger.info(f"Getting file size for {url} using requests")
        response = requests.head(url, timeout=30)
        
        if response.status_code == 200 and 'Content-Length' in response.headers:
            size = int(response.headers['Content-Length'])
            logger.info(f"Remote file size: {size/MB:.2f} MB")
            return size
        else:
            logger.debug(f"Content-Length header not found in response, falling back to curl")
            cmd = ["curl", "-sI", url]
            process = subprocess.Popen(cmd, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
            stdout, stderr = process.communicate()
            
            if process.returncode == 0:
                for line in stdout.decode().split('\n'):
                    if line.lower().startswith('content-length:'):
                        size = int(line.split(':', 1)[1].strip())
                        logger.info(f"Remote file size from curl: {size/MB:.2f} MB")
                        return size
        
        logger.error(f"Failed to get Content-Length from server for {url}")
        return None
    except Exception as e:
        logger.error(f"Error getting file size: {str(e)}")
        if DEBUG:
            logger.debug(traceback.format_exc())
        return None



def download_chunk_to_memory(url, start_byte, end_byte, progress_callback=None):
    """Download a chunk directly into memory without saving to a file
    
    Args:
        url: URL to download from
        start_byte: Start byte position
        end_byte: End byte position
        progress_callback: Optional callback function for progress updates
        
    Returns:
        Tuple of (success, result) where result is either the bytes data or error message
    """
    chunk_size = end_byte - start_byte + 1
    logger.info(f"Downloading chunk to memory: bytes {start_byte}-{end_byte} ({chunk_size/MB:.2f} MB)")
    
    try:
        headers = {'Range': f'bytes={start_byte}-{end_byte}'}
        logger.debug(f"Requesting byte range: {headers['Range']}")
        
        with requests.get(url, headers=headers, stream=False, timeout=60) as response:
            if response.status_code not in (200, 206):
                error_msg = f"HTTP error: {response.status_code}"
                logger.error(error_msg)
                return False, error_msg
            
            # Get data directly
            data = response.content
            
            # Call progress callback if provided
            if progress_callback:
                progress_callback(len(data))
            
            logger.info(f"Successfully downloaded {len(data)/MB:.2f} MB to memory")
            return True, data
            
    except Exception as e:
        error_msg = f"Exception during download: {str(e)}"
        logger.error(error_msg)
        if DEBUG:
            logger.debug(traceback.format_exc())
            
        return False, error_msg





def get_osm_config(extract):
    """Get URL and filename for a specific OSM extract"""
    if extract == 'france':
        return {
            'url': 'https://download.geofabrik.de/europe/france-latest.osm.pbf',
            'filename': 'france-latest.osm.pbf'
        }
    elif extract == 'planet':
        return {
            'url': 'https://planet.openstreetmap.org/pbf/planet-latest.osm.pbf',
            'filename': 'planet-latest.osm.pbf'
        }
    else:
        logger.error(f'Error: Invalid extract "{extract}"')
        logger.error('Valid options are "france" or "planet"')
        return None
