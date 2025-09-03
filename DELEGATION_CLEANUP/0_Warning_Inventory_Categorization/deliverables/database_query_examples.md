# Database Query Examples

## Overview

The `warning_database.json` file contains 736 structured warning entries with comprehensive analysis metadata. This guide provides query examples for filtering, analyzing, and managing the warning database.

## Database Structure Quick Reference

```json
{
  "metadata": {...},
  "summary_statistics": {...},
  "warnings": [
    {
      "warning_id": "W001",
      "source_info": {"file_path": "...", "line_number": 14, ...},
      "classification": {"final_classification": "FALSE_POSITIVE", "confidence_level": 0.95},
      "category_analysis": {"primary_category": "MESI_COHERENCE_PROTOCOL", ...},
      "resolution_tracking": {"assigned_milestone": "MILESTONE_4", ...}
    }
  ]
}
```

## Command Line Query Examples

### Using jq (JSON Query Tool)

#### 1. Basic Statistics
```bash
# Total warning count
jq '.metadata.total_warnings' warning_database.json

# Classification summary
jq '.summary_statistics.by_classification' warning_database.json

# Category distribution
jq '.summary_statistics.by_category' warning_database.json
```

#### 2. Filter by Classification
```bash
# All false positive warnings
jq '.warnings[] | select(.classification.final_classification == "FALSE_POSITIVE")' warning_database.json

# High confidence warnings (≥90%)
jq '.warnings[] | select(.classification.confidence_level >= 0.9)' warning_database.json

# Medium confidence warnings (70-90%)
jq '.warnings[] | select(.classification.confidence_level >= 0.7 and .classification.confidence_level < 0.9)' warning_database.json
```

#### 3. Filter by Category
```bash
# MESI coherence protocol warnings
jq '.warnings[] | select(.category_analysis.primary_category == "MESI_COHERENCE_PROTOCOL")' warning_database.json

# ML feature analysis warnings
jq '.warnings[] | select(.category_analysis.primary_category == "ML_FEATURE_ANALYSIS")' warning_database.json

# Telemetry collection warnings
jq '.warnings[] | select(.category_analysis.primary_category == "TELEMETRY_COLLECTION")' warning_database.json
```

#### 4. Filter by File/Module
```bash
# Warnings in coherence module
jq '.warnings[] | select(.source_info.file_path | contains("coherence"))' warning_database.json

# Warnings in eviction module
jq '.warnings[] | select(.source_info.file_path | contains("eviction"))' warning_database.json

# Warnings in specific file
jq '.warnings[] | select(.source_info.file_path == "src/cache/coherence/data_structures.rs")' warning_database.json
```

#### 5. Filter by Milestone Assignment
```bash
# MILESTONE_4 assignments
jq '.warnings[] | select(.resolution_tracking.assigned_milestone == "MILESTONE_4")' warning_database.json

# High priority warnings
jq '.warnings[] | select(.resolution_tracking.priority == "high")' warning_database.json

# Pending resolution status
jq '.warnings[] | select(.resolution_tracking.status == "pending")' warning_database.json
```

#### 6. Complex Queries
```bash
# High-confidence false positives in coherence module
jq '.warnings[] | select(.classification.final_classification == "FALSE_POSITIVE" and .classification.confidence_level >= 0.9 and (.source_info.file_path | contains("coherence")))' warning_database.json

# Field-level warnings with suppress recommendations
jq '.warnings[] | select(.warning_details.warning_code == "unused_field") | {warning_id, file: .source_info.file_path, annotation: .annotation_metadata.suggested_allow_directive}' warning_database.json

# Method-level warnings by category
jq '.warnings[] | select(.warning_details.warning_code == "unused_method") | group_by(.category_analysis.primary_category) | map({category: .[0].category_analysis.primary_category, count: length})' warning_database.json
```

## Python Query Examples

### Basic Database Loading
```python
import json
from typing import List, Dict, Any

# Load database
with open('warning_database.json', 'r') as f:
    db = json.load(f)

warnings = db['warnings']
print(f"Total warnings: {len(warnings)}")
```

### Filter Functions
```python
def filter_by_classification(warnings: List[Dict], classification: str) -> List[Dict]:
    """Filter warnings by final classification"""
    return [w for w in warnings if w['classification']['final_classification'] == classification]

def filter_by_confidence(warnings: List[Dict], min_confidence: float) -> List[Dict]:
    """Filter warnings by minimum confidence level"""
    return [w for w in warnings if w['classification']['confidence_level'] >= min_confidence]

def filter_by_category(warnings: List[Dict], category: str) -> List[Dict]:
    """Filter warnings by primary category"""
    return [w for w in warnings if w['category_analysis']['primary_category'] == category]

def filter_by_file_pattern(warnings: List[Dict], pattern: str) -> List[Dict]:
    """Filter warnings by file path pattern"""
    return [w for w in warnings if pattern in w['source_info']['file_path']]

def filter_by_milestone(warnings: List[Dict], milestone: str) -> List[Dict]:
    """Filter warnings by assigned milestone"""
    return [w for w in warnings if w['resolution_tracking']['assigned_milestone'] == milestone]
```

### Analysis Examples
```python
# High-confidence MESI coherence warnings
mesi_warnings = filter_by_category(warnings, 'MESI_COHERENCE_PROTOCOL')
high_conf_mesi = filter_by_confidence(mesi_warnings, 0.9)
print(f"High-confidence MESI warnings: {len(high_conf_mesi)}")

# Warnings by file with counts
from collections import Counter
file_counts = Counter(w['source_info']['file_path'] for w in warnings)
top_files = file_counts.most_common(10)
print("Files with most warnings:")
for file, count in top_files:
    print(f"  {file}: {count}")

# Category distribution
category_counts = Counter(w['category_analysis']['primary_category'] for w in warnings)
print("Category distribution:")
for category, count in category_counts.items():
    print(f"  {category}: {count}")
```

### Generate Reports
```python
def generate_file_report(warnings: List[Dict], output_file: str):
    """Generate per-file warning report"""
    from collections import defaultdict
    
    file_warnings = defaultdict(list)
    for warning in warnings:
        file_path = warning['source_info']['file_path']
        file_warnings[file_path].append(warning)
    
    with open(output_file, 'w') as f:
        for file_path, file_warns in sorted(file_warnings.items()):
            f.write(f"## {file_path}\n")
            f.write(f"Warnings: {len(file_warns)}\n\n")
            
            for warning in file_warns:
                f.write(f"- Line {warning['source_info']['line_number']}: {warning['warning_details']['message']}\n")
                f.write(f"  Category: {warning['category_analysis']['primary_category']}\n")
                f.write(f"  Confidence: {warning['classification']['confidence_level']:.2f}\n\n")

def generate_milestone_assignments(warnings: List[Dict], output_file: str):
    """Generate milestone assignment summary"""
    from collections import defaultdict
    
    milestone_warnings = defaultdict(list)
    for warning in warnings:
        milestone = warning['resolution_tracking']['assigned_milestone']
        milestone_warnings[milestone].append(warning)
    
    with open(output_file, 'w') as f:
        f.write("# Milestone Assignment Summary\n\n")
        for milestone, warns in sorted(milestone_warnings.items()):
            f.write(f"## {milestone}\n")
            f.write(f"Assigned warnings: {len(warns)}\n")
            
            # Count by priority
            priority_counts = Counter(w['resolution_tracking']['priority'] for w in warns)
            f.write(f"Priority breakdown: {dict(priority_counts)}\n\n")
```

## Database Maintenance Queries

### Update Resolution Status
```python
def update_warning_status(warning_id: str, new_status: str):
    """Update resolution status for specific warning"""
    for warning in warnings:
        if warning['warning_id'] == warning_id:
            warning['resolution_tracking']['status'] = new_status
            warning['timestamps']['last_modified'] = datetime.now().strftime("%Y-%m-%d")
            break

def bulk_update_milestone_status(milestone: str, new_status: str):
    """Bulk update status for all warnings in milestone"""
    count = 0
    for warning in warnings:
        if warning['resolution_tracking']['assigned_milestone'] == milestone:
            warning['resolution_tracking']['status'] = new_status
            count += 1
    return count
```

### Progress Tracking
```python
def get_progress_summary() -> Dict[str, Any]:
    """Get overall progress summary"""
    total = len(warnings)
    completed = len([w for w in warnings if w['resolution_tracking']['status'] == 'completed'])
    in_progress = len([w for w in warnings if w['resolution_tracking']['status'] == 'in_progress'])
    pending = len([w for w in warnings if w['resolution_tracking']['status'] == 'pending'])
    
    return {
        'total_warnings': total,
        'completed': completed,
        'in_progress': in_progress,
        'pending': pending,
        'completion_percentage': (completed / total * 100) if total > 0 else 0
    }

def get_milestone_progress(milestone: str) -> Dict[str, Any]:
    """Get progress summary for specific milestone"""
    milestone_warnings = [w for w in warnings if w['resolution_tracking']['assigned_milestone'] == milestone]
    total = len(milestone_warnings)
    completed = len([w for w in milestone_warnings if w['resolution_tracking']['status'] == 'completed'])
    
    return {
        'milestone': milestone,
        'total_warnings': total,
        'completed': completed,
        'remaining': total - completed,
        'completion_percentage': (completed / total * 100) if total > 0 else 0
    }
```

## Validation Queries

### Data Integrity Checks
```python
def validate_database_integrity(db: Dict[str, Any]) -> List[str]:
    """Validate database integrity and return list of issues"""
    issues = []
    warnings = db['warnings']
    
    # Check for duplicate warning IDs
    warning_ids = [w['warning_id'] for w in warnings]
    if len(warning_ids) != len(set(warning_ids)):
        issues.append("Duplicate warning IDs detected")
    
    # Validate confidence levels
    invalid_confidence = [w['warning_id'] for w in warnings 
                         if not (0.0 <= w['classification']['confidence_level'] <= 1.0)]
    if invalid_confidence:
        issues.append(f"Invalid confidence levels: {invalid_confidence}")
    
    # Validate file paths exist (requires filesystem check)
    # missing_files = [w['warning_id'] for w in warnings 
    #                 if not Path(w['source_info']['file_path']).exists()]
    
    return issues
```

## Export Functions

### CSV Export
```python
import csv

def export_to_csv(warnings: List[Dict], filename: str):
    """Export warnings to CSV format"""
    fieldnames = [
        'warning_id', 'file_path', 'line_number', 'message',
        'primary_category', 'final_classification', 'confidence_level',
        'assigned_milestone', 'resolution_status'
    ]
    
    with open(filename, 'w', newline='') as csvfile:
        writer = csv.DictWriter(csvfile, fieldnames=fieldnames)
        writer.writeheader()
        
        for warning in warnings:
            writer.writerow({
                'warning_id': warning['warning_id'],
                'file_path': warning['source_info']['file_path'],
                'line_number': warning['source_info']['line_number'],
                'message': warning['warning_details']['message'],
                'primary_category': warning['category_analysis']['primary_category'],
                'final_classification': warning['classification']['final_classification'],
                'confidence_level': warning['classification']['confidence_level'],
                'assigned_milestone': warning['resolution_tracking']['assigned_milestone'],
                'resolution_status': warning['resolution_tracking']['status']
            })
```

## Usage Tips

1. **Performance**: For large queries, consider using generators or itertools for memory efficiency
2. **Backup**: Always backup the database before making modifications
3. **Validation**: Run integrity checks after manual database modifications
4. **Automation**: Create scripts for common query patterns to streamline analysis
5. **Documentation**: Update query examples as database schema evolves

These examples provide comprehensive access to the warning database for analysis, reporting, and management throughout the milestone execution process.