#!/usr/bin/env bash

set -o errexit
set -o nounset
set -o pipefail

PACKET_FILTER='_ws.col.protocol == "USB-PTP" && usb.endpoint_address.direction == "Out"'

parsing=""
profile=""
start_tx_id_num=-1

print_le2() {
  chunk="$1"
  c0="${chunk:0:2}"
  c1="${chunk:2:2}"
  echo -n "$c1$c0 "
}

print_le4() {
  chunk="$1"
  c0="${chunk:0:2}"
  c1="${chunk:2:2}"
  c2="${chunk:4:2}"
  c3="${chunk:6:2}"
  echo -n "$c3$c2$c1$c0 "
}

print_profile() {
  local profile="$1"
  local pos=0

  # Number of fields
  print_le2 "${profile:pos:4}"
  pos=$((pos + 4))

  echo -n "- "

  # Identifier length (known)
  pos=$((pos + 2))

  # Identifier
  for _ in {0..7}; do
    print_le2 "${profile:pos:4}"
    pos=$((pos + 4))
  done

  # Buffer
  zeros_len=$((0x1EE * 2))
  echo -n "[...] "
  pos=$((pos + zeros_len))

  # Fields
  while (( pos < ${#profile} )); do
    print_le4 "${profile:pos:8}"
    pos=$((pos + 8))
  done

  echo
}

while IFS=$'|' read -r cont_type op_code tx_id device_prop payload frame; do
  cont_type=${cont_type:-}
  op_code=${op_code:-}
  tx_id=${tx_id:-0}
  tx_id_num=$((16#${tx_id#0x}))
  device_prop=${device_prop:-}
  payload=${payload:-}
  frame=${frame:-}

  if [[ -n "$parsing" ]] && [[ "$tx_id_num" -eq $((start_tx_id_num + 1)) ]]; then
    print_profile "$profile"
    parsing=""
    profile=""
    start_tx_id_num=""
  fi

  if [[ "$op_code" == "0x1016" ]] && [[ "$device_prop" == "0xd185" ]]; then
    parsing="true"
    start_tx_id_num=$tx_id_num
  fi

  if [[ -n "$parsing" ]]; then
    # Fuji sometimes sends back-to-back packets with an invalid header.
    # If any of the below are true while parsing, we are hitting this case,
    # and we need to take the bytes from the "header" and prepend them to the real payload.
    if [[ ( "$cont_type" != "0x0001" && "$cont_type" != "0x0002" ) || "$op_code" != "0x1016" || "$tx_id_num" -ne "$start_tx_id_num" ]]; then
      # Take bytes 64-75 of frame (chars 128-151, since 2 chars per byte)
      frame_part="${frame:128:24}"
      payload="$frame_part$payload"
    fi

    profile+="$payload"
  fi
done < <(
  tshark -Q -l -i "$INTERFACE" \
    -Y "$PACKET_FILTER" \
    -T fields \
    -E separator=\| \
    -e usb-ptp.container.type \
    -e usb-ptp.operation.code \
    -e usb-ptp.transaction.id \
    -e usb-ptp.device.property \
    -e usb-ptp.payload \
    -e frame.raw
)
