Loading `curator` with an incorrect URL resulted in the UI incrementally appending lines to the bottom
of the screen and shifted the UI.


Bottom of the screen looked like this:

```
┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┛└────────────────────────────────────────────────────────┘└────────────────────────────────────────────────────────┘
 q quit  1/2/3 views  h/l ← → column  j/k ↑ ↓ navigate  r reloadsse: stream error (attempt 2): sse: server returned 404 Not Found
                                                                                                                                 sse: stream error (attempt 3): sse: server returned 404 Not Found
                    sse: stream error (attempt 4): sse: server returned 404 Not Found
                                                                                     sse: stream error (attempt 5): sse: server returned 404 Not Found
                                                                                                                                                      sse: stream error (attempt 6): sse: server returned 404 Not Found

```
