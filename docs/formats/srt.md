# Walksnail SRT files

The goggles write an SRT subtitle file next to each video. Each cue holds the link data for its moment:

```text
1
00:00:00,000 --> 00:00:00,150
Signal:4 CH:7 FlightTime:0 SBat:15.9V GBat:7.8V Delay:37ms Bitrate:25.0Mbps Distance:2m
```

| Field | Meaning | Unit in the text |
|-------|---------|------------------|
| `Signal` | Signal level (required to read the line) | none |
| `CH` | Video channel | none |
| `FlightTime` | Flight time | seconds |
| `SBat` | Aircraft battery voltage | V |
| `GBat` | Goggles battery voltage | V |
| `Delay` | Video latency | ms |
| `Bitrate` | Video bitrate | Mbps |
| `Distance` | Distance from the goggles | m |

Lines in the debug format of newer firmware (`CH:1 MCS:4 SP[...]`) have no `Signal` field and are not data.
Lines are read with the units removed. A missing field is zero. Files may use Windows line endings.
