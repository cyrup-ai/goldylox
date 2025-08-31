#!/bin/bash

echo "=== DUPLICATE TYPE ANALYSIS ==="
echo

# Get all duplicate type names (count > 1)
DUPLICATES=$(grep -r "^[[:space:]]*\(pub[[:space:]]\+\)\?\(struct\|enum\|trait\)[[:space:]]\+\([A-Za-z_][A-Za-z0-9_]*\)" src/ | \
    sed -E 's/.*\b(struct|enum|trait)[[:space:]]+([A-Za-z_][A-Za-z0-9_]*).*/\2/' | \
    sort | uniq -c | sort -nr | awk '$1 > 1 {print $2}')

for TYPE in $DUPLICATES; do
    echo "=== $TYPE ==="
    
    # Find all definitions
    echo "DEFINITIONS:"
    grep -rn "^[[:space:]]*\(pub[[:space:]]\+\)\?\(struct\|enum\|trait\)[[:space:]]\+$TYPE" src/
    echo
    
    # Count all references
    USAGE_COUNT=$(grep -r "$TYPE" src/ | wc -l | tr -d ' ')
    echo "TOTAL USAGE COUNT: $USAGE_COUNT"
    echo
    echo "FILES WITH REFERENCES:"
    grep -r "$TYPE" src/ | cut -d: -f1 | sort | uniq -c | sort -nr
    echo
    echo "==============================================="
    echo
done