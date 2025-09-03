# Warning Database Schema

## Overview
Structured database schema for systematic tracking and management of all 737 compiler warnings with complete analysis metadata and resolution progress tracking.

## Database Structure

### Root Level
```json
{
  "metadata": {
    "generated_at": "2025-01-02T10:30:00Z",
    "total_warnings": 737,
    "schema_version": "1.0",
    "analysis_completion_date": "2025-01-02",
    "goldylox_commit_hash": "current",
    "analysis_milestone": "M0-T5"
  },
  "summary_statistics": {
    "by_classification": {...},
    "by_category": {...},
    "by_confidence": {...},
    "by_milestone": {...}
  },
  "warnings": [...]
}
```

### Warning Entry Schema
Each warning entry contains comprehensive metadata from all analysis tasks:

```json
{
  "warning_id": "W001",
  "source_info": {
    "file_path": "src/cache/coherence/data_structures.rs",
    "line_number": 30,
    "column": null,
    "function_context": "CoherenceController",
    "module_path": "goldylox::cache::coherence::data_structures"
  },
  "warning_details": {
    "warning_type": "dead_code",
    "warning_code": "unused_field", 
    "message": "field is never read: `protocol_config`",
    "severity": "warning",
    "raw_output": "warning: field is never read..."
  },
  "category_analysis": {
    "primary_category": "MESI_COHERENCE_PROTOCOL",
    "subcategory": "protocol_configuration",
    "pattern_type": "field_ownership",
    "complexity_level": "high",
    "architectural_significance": "critical"
  },
  "integration_analysis": {
    "integration_chain": [
      "UnifiedCacheManager.tier_operations",
      "TierOperations.coherence_controller", 
      "CoherenceController.protocol_config"
    ],
    "usage_evidence": [
      {
        "location": "src/cache/coherence/data_structures.rs:338",
        "usage_type": "field_initialization",
        "confidence": "direct"
      }
    ],
    "ownership_pattern": "nested_ownership",
    "crossbeam_integration": true
  },
  "architecture_analysis": {
    "pattern_category": "complex_ownership",
    "architectural_patterns": [
      "trait_object_dynamic_dispatch",
      "factory_method_construction",
      "crossbeam_messaging"
    ],
    "compiler_limitation": "static_analysis_limitation",
    "design_sophistication": "high"
  },
  "classification": {
    "final_classification": "FALSE_POSITIVE",
    "confidence_level": 0.95,
    "reasoning": "Used in protocol validation and configuration management",
    "evidence_strength": "strong",
    "validation_method": "source_code_analysis"
  },
  "resolution_tracking": {
    "assigned_milestone": "MILESTONE_4",
    "assigned_task": "M4-T2",
    "resolution_strategy": "suppress_with_annotation",
    "priority": "low",
    "estimated_effort": "trivial",
    "dependencies": [],
    "status": "pending"
  },
  "annotation_metadata": {
    "suggested_allow_directive": "#[allow(dead_code)]",
    "suggested_comment": "MESI coherence - used in protocol validation and configuration management",
    "suppress_scope": "field_level",
    "requires_documentation": false
  },
  "timestamps": {
    "first_detected": "2025-01-01",
    "last_analyzed": "2025-01-02",
    "resolution_target": "2025-01-03"
  }
}
```

## Field Definitions

### Core Identification
- `warning_id`: Unique identifier (W001, W002, etc.)
- `source_info`: Complete location and context information
- `warning_details`: Raw warning information from compiler

### Analysis Results (from M0-T1 through M0-T4)
- `category_analysis`: Results from warning categorization (M0-T1)
- `integration_analysis`: Results from integration chain analysis (M0-T2)  
- `architecture_analysis`: Results from architecture pattern recognition (M0-T3)
- `classification`: Results from false positive classification (M0-T4)

### Resolution Management
- `resolution_tracking`: Assignment to milestones and progress tracking
- `annotation_metadata`: Specific suppression strategy details
- `timestamps`: Timeline tracking for resolution progress

## Query Capabilities

### By Classification
```json
{
  "FALSE_POSITIVE": [...],
  "POTENTIAL_DEAD_CODE": [...],
  "CONFIRMED_DEAD_CODE": [...]
}
```

### By Category
```json
{
  "MESI_COHERENCE_PROTOCOL": [...],
  "ML_FEATURE_ANALYSIS": [...],
  "ATOMIC_STATE_MANAGEMENT": [...]
}
```

### By Confidence Level
```json
{
  "HIGH_CONFIDENCE": [...],    // 0.9+
  "MEDIUM_CONFIDENCE": [...],  // 0.7-0.9
  "LOW_CONFIDENCE": [...]      // <0.7
}
```

### By Milestone Assignment
```json
{
  "MILESTONE_1": [...],
  "MILESTONE_4": [...],
  "UNASSIGNED": [...]
}
```

## Summary Statistics Schema
```json
{
  "by_classification": {
    "FALSE_POSITIVE": {"count": 700, "percentage": 95.0},
    "POTENTIAL_DEAD_CODE": {"count": 37, "percentage": 5.0}
  },
  "by_category": {
    "MESI_COHERENCE_PROTOCOL": {"count": 245, "percentage": 33.2},
    "ML_FEATURE_ANALYSIS": {"count": 156, "percentage": 21.2}
  },
  "by_confidence": {
    "HIGH_CONFIDENCE": {"count": 663, "percentage": 90.0},
    "MEDIUM_CONFIDENCE": {"count": 59, "percentage": 8.0},
    "LOW_CONFIDENCE": {"count": 15, "percentage": 2.0}
  },
  "by_milestone": {
    "MILESTONE_1": {"count": 5, "percentage": 0.7},
    "MILESTONE_4": {"count": 700, "percentage": 95.0},
    "UNASSIGNED": {"count": 32, "percentage": 4.3}
  }
}
```

## Data Validation Rules

### Required Fields
- All warnings must have: `warning_id`, `source_info`, `warning_details`, `classification`
- All classifications must have: `final_classification`, `confidence_level`, `reasoning`

### Consistency Checks
- `confidence_level` must be between 0.0 and 1.0
- `final_classification` must be from allowed enum values
- `file_path` must exist in codebase
- `assigned_milestone` must reference valid milestone

### Data Integrity
- No duplicate `warning_id` values
- All milestone references must be valid
- Sum of category counts must equal total warning count

## Usage Examples

### Find High-Confidence False Positives
```javascript
warnings.filter(w => 
  w.classification.final_classification === "FALSE_POSITIVE" && 
  w.classification.confidence_level >= 0.9
)
```

### Find Warnings by File
```javascript
warnings.filter(w => 
  w.source_info.file_path.includes("coherence")
)
```

### Find Unresolved High-Priority Items
```javascript
warnings.filter(w => 
  w.resolution_tracking.status === "pending" && 
  w.resolution_tracking.priority === "high"
)
```

This schema provides complete traceability from raw warnings through analysis to resolution, enabling systematic warning management across all milestones.