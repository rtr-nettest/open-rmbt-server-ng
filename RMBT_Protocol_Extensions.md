# RTR Multithreaded Broadband Test (RMBT): Protocol Extensions

**Version 1.0.0, 27.01.2025**

This document extends the [RMBT Specification](https://github.com/rtr-nettest/rmbt-server/blob/master/RMBT_specification.md) with additional protocol commands that provide enhanced measurement capabilities and result integrity verification.

## Overview

The following commands extend the standard RMBT protocol to provide:
- Enhanced upload measurement with controlled timing feedback to avoid channel congestion
- Cryptographic result integrity verification
- Additional measurement data collection

### Key Differences from Standard PUT

| Feature | Standard PUT | PUTTIMERESULT |
|---------|--------------|---------------|
| **Timing Reports** | `TIME <t> BYTES <b>` after each chunk | `TIME <t> BYTES <b>` at specified intervals or end of test |
| **Channel Congestion** | High (frequent reports) | Low (controlled reporting) |
| **Measurement Accuracy** | May be affected by protocol overhead | Cleaner measurements |
| **Real-time Display** | Difficult due to irregular timing | Ideal for graphs |
| **Protocol Overhead** | High during transmission | Minimal during transmission |
| **Final Summary** | No summary report | `TIMERESULT` with all measurements |

## Extended Communication Protocol

### Client Commands

#### PUTTIMERESULT \<CHUNKSIZE\> [\<INTERVAL\>]
The client requests to send a data stream to the RMBT Server consisting of chunks of the previously specified size $s$ or the size optionally given in the \<CHUNKSIZE> argument, with periodic timing feedback to avoid channel congestion.

**Behavior:**
- The server responds with [OK](#ok)
- The client sends data chunks continuously for the test duration
- The server collects timing measurements internally but sends [TIME \<t\> BYTES \<b\>](#time-t-bytes-b) reports at specified intervals (if \<INTERVAL\> is provided) or collects all measurements for end-of-test reporting
- The data stream ends with a [termination byte](#termination-byte) on the last position of the transmitted chunks
- After receiving the last chunk, the server sends a [TIMERESULT](#timeresult-t1-b1-t2-b2-tn-bn) message containing all timing measurements
- The client responds with [OK](#ok) to acknowledge receipt of the timing results

**Timing Measurements:**
The server measures and reports:
- \<t>: the number of nanoseconds passed since receiving the PUTTIMERESULT command
- \<b>: the cumulative number of bytes received at each measurement point
- \<INTERVAL\>: (optional) minimum time interval in milliseconds between timing reports to reduce channel congestion

**Format:**
```
PUTTIMERESULT [CHUNKSIZE] [INTERVAL]
```

**Parameters:**
- \<CHUNKSIZE\>: (optional) Size of data chunks in bytes. If not specified, uses the previously set chunk size.
- \<INTERVAL\>: (optional) Minimum time interval in milliseconds between timing reports. If not specified, all measurements are collected and sent at the end.

**Example without interval (all measurements at end):**
```
Client -> Server: PUTTIMERESULT 131072
Server -> Client: OK
Client -> Server: <CHUNKS...>
Client -> Server: <CHUNKS...>
Client -> Server: <ENDCHUNK>
Server -> Client: TIMERESULT (62318418 131072); (124636836 262144); (186955254 393216); (249273672 524288)
Client -> Server: OK
```

#### SIGNEDRESULT
The client requests a cryptographically signed result envelope from the RMBT Server containing all measurement data and integrity verification.

**Behavior:**
- The server generates a comprehensive measurement summary
- The server creates a cryptographic signature using HMAC-SHA256
- The server sends the signed envelope to the client
- The client responds with [OK](#ok) to acknowledge receipt

**Signed Data Includes:**
- GETTIME measurements (bytes received, time)
- PUTTIMERESULT measurements (bytes sent, time)
- Client IP address
- Timestamp
- Cryptographic signature

**Format:**
```
SIGNEDRESULT
```

**Example:**
```
Client -> Server: SIGNEDRESULT
Server -> Client: GETTIME:(15073280 7047485393); PUTTIMERESULT:(1048576 825324442); CLIENT_IP:192.168.1.100; TIMESTAMP:1234567890;:aGVsbG8gd29ybGQ=
Client -> Server: OK
```

### SIGNEDRESULT Security
- Uses industry-standard HMAC-SHA256 algorithm
- Generates unique keys per measurement session
- Provides tamper-evident measurement results
- Enables third-party verification of test results

## Compatibility

These protocol extensions are designed to be backward compatible with the standard RMBT protocol:
- Existing clients can ignore unknown commands
- New commands follow the same message format conventions
- Server responses maintain the same structure
- No changes to existing protocol phases are required

## References

- [RMBT Specification v1.1.1](https://github.com/rtr-nettest/rmbt-server/blob/master/RMBT_specification.md)
- RFC 2104: HMAC: Keyed-Hashing for Message Authentication
- RFC 6234: US Secure Hash Algorithms (SHA and SHA-based HMAC and HKDF)
