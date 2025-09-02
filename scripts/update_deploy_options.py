#!/usr/bin/env python3
"""
Script to update release options in deploy-server.yml workflow
"""

import re
import sys
import os

def update_deploy_workflow(releases_list):
    """Update the deploy-server.yml workflow with new release options"""
    
    workflow_path = '.github/workflows/deploy-server.yml'
    
    if not os.path.exists(workflow_path):
        print(f"Error: {workflow_path} not found")
        return False
    
    # Read current workflow file
    with open(workflow_path, 'r') as f:
        content = f.read()
    
    # Create options list
    options_lines = ['          - \'latest\'']
    for release in releases_list:
        if release.strip():
            options_lines.append(f"          - '{release.strip()}'")
    
    options_text = '\n'.join(options_lines)
    
    # Replace the options section in workflow file
    pattern = r'(\s+options:\s*\n)(\s+- \'[^\']+\'\s*\n)*'
    replacement = f'\\1{options_text}\n'
    
    new_content = re.sub(pattern, replacement, content)
    
    # Write updated file
    with open(workflow_path, 'w') as f:
        f.write(new_content)
    
    print(f"Updated {workflow_path} with {len(releases_list)} releases")
    return True

if __name__ == '__main__':
    if len(sys.argv) < 2:
        print("Usage: python3 update_deploy_options.py <releases_list>")
        print("Example: python3 update_deploy_options.py 'v1.0.0\nv2.0.0\nv3.0.0'")
        sys.exit(1)
    
    releases = sys.argv[1].strip().split('\n')
    success = update_deploy_workflow(releases)
    sys.exit(0 if success else 1)
