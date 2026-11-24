# AccessKit Windows adapter

This is the Windows adapter for [AccessKit](https://accesskit.dev/). It exposes an AccessKit accessibility tree through the UI Automation API.

The Open GPUI workspace patches AccessKit Windows 0.32.1 from
`1bbcf100942bac96c2c3a4a91cb67b0b20201a24`. Its UI Automation geometry
providers return `UIA_E_ELEMENTNOTAVAILABLE` if Win32 cannot convert client
coordinates after the HWND is destroyed. Replace this workspace patch once an
upstream release handles the same provider lifetime race.
