# Clock quadrant distribution


## The `CLKC` clock center tile

The `CLKC` tile is located in the center of the FPGA (intersection of primary vertical and horizontal clock spines) of all devices except `xc3s50a`. It has permanent buffers forwarding the clock signals from `CLKB` and `CLKT` to `GCLKVM`. It has no configuration.

TODO: describe exact forwarding

{{tile spartan3 CLKC}}


## The `CLKC_50A` clock center tile

TODO: document

{{tile spartan3 CLKC_50A}}


## The `GCLKVM` secondary clock center tiles

The `GCLKVM` tiles are located on the intersection of secondary vertical clock spines and the horizontal clock spine.

TODO: document 


{{tile spartan3 GCLKVM.S3}}
{{tile spartan3 GCLKVM.S3E}}


## The `GCLKVC` clock spine distribution tiles

TODO: document

{{tile spartan3 GCLKVC}}
