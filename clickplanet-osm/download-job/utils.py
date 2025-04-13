#!/usr/bin/env python3
"""
Utilities for OSM data processing

Shared utilities, constants, and logging configuration for the OSM download job.
"""

import os
import sys
import logging
import platform
import shutil
from datetime import datetime

KB = 1024
MB = KB * 1024
GB = MB * 1024

DEFAULT_CHUNK_SIZE = 1 * GB
DEBUG = os.environ.get('DEBUG', 'false').lower() in ('true', '1', 'yes')
SKIP_CLEANUP = os.environ.get('SKIP_CLEANUP', 'false').lower() in ('true', '1', 'yes')
TMP_DIR = os.environ.get('TMP_DIR', os.path.join(os.getcwd(), 'tmp'))
ARIA2_SESSION_DIR = os.environ.get('ARIA2_SESSION_DIR', os.path.join(TMP_DIR, 'aria2_session'))

def setup_logging():
    log_level = logging.DEBUG if DEBUG else logging.INFO
    log_format = '%(asctime)s [%(levelname)s] %(message)s'
    date_format = '%Y-%m-%d %H:%M:%S'
    
    logging.basicConfig(level=log_level, format=log_format, datefmt=date_format)
    
    logger = logging.getLogger('osm_downloader')
    
    logger.info(f"Starting OSM Data Chunked Downloader at {datetime.now()}")
    logger.info(f"System: {platform.system()} {platform.release()} ({platform.machine()})")
    logger.info(f"Python: {sys.version.split()[0]}")
    logger.info(f"Debug mode: {'Enabled' if DEBUG else 'Disabled'}")
    logger.info(f"Using temporary directory: {TMP_DIR}")
    
    return logger

logger = setup_logging()

def ensure_directory_exists(directory_path):
    os.makedirs(directory_path, exist_ok=True)
    logger.debug(f"Ensured directory exists: {directory_path}")
    
def create_temp_directory():
    temp_dir = os.path.join(TMP_DIR, f"osm_download_{int(datetime.now().timestamp())}")
    ensure_directory_exists(temp_dir)
    logger.info(f"Created dedicated temp directory: {temp_dir}")
    return temp_dir

def cleanup_all_temp_directories(temp_dir):
    if SKIP_CLEANUP:
        logger.info("SKIP_CLEANUP is enabled, keeping temporary directories")
        logger.info(f"Temporary directory location: {temp_dir}")
        return
    
    try:
        logger.info("Final cleanup - ensuring all temporary directories are removed")
        
        if os.path.exists(temp_dir):
            logger.info(f"Cleaning up temporary directory: {temp_dir}")
            shutil.rmtree(temp_dir, ignore_errors=True)
        
        for item in os.listdir(TMP_DIR):
            path = os.path.join(TMP_DIR, item)
            if item.startswith("osm_download_") and os.path.isdir(path):
                logger.info(f"Cleaning up additional osm directory: {path}")
                shutil.rmtree(path, ignore_errors=True)
                
        logger.info("All temporary files have been cleaned up.")
    except Exception as cleanup_error:
        logger.error(f"Warning: Could not fully clean up temporary directories: {cleanup_error}")

ensure_directory_exists(TMP_DIR)
