#!/bin/sh

# Print physical iPhone/iPad identifiers from devicectl's documented JSON
# output. Keeping this separate makes the demo installer's device selection
# testable without building or installing the app.
list_physical_ios_device_ids_from_json() {
    device_json=$1
    device_index=0

    while device_id=$(/usr/bin/plutil \
        -extract "result.devices.$device_index.identifier" raw -o - \
        "$device_json" 2>/dev/null); do
        device_type=$(/usr/bin/plutil \
            -extract "result.devices.$device_index.hardwareProperties.deviceType" raw -o - \
            "$device_json" 2>/dev/null || true)
        device_reality=$(/usr/bin/plutil \
            -extract "result.devices.$device_index.hardwareProperties.reality" raw -o - \
            "$device_json" 2>/dev/null || true)

        case "$device_type:$device_reality" in
            iPhone:physical|iPad:physical)
                printf '%s\n' "$device_id"
                ;;
        esac

        device_index=$((device_index + 1))
    done
}

list_reachable_ios_device_ids() {
    reachable_device_json=$(mktemp -t riot-demo-devices)

    if ! xcrun devicectl list devices \
        --filter "(hardwareProperties.deviceType == 'iPhone' OR hardwareProperties.deviceType == 'iPad') AND (State BEGINSWITH 'available' OR State BEGINSWITH 'connected')" \
        --json-output "$reachable_device_json" \
        --quiet >/dev/null 2>&1; then
        rm -f "$reachable_device_json"
        return 1
    fi

    list_physical_ios_device_ids_from_json "$reachable_device_json"
    device_list_status=$?
    rm -f "$reachable_device_json"
    return "$device_list_status"
}
