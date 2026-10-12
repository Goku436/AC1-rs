@echo off
rem Play back an ac1-rs replay: drag a replays\ac1-replay-*.bin file onto this, or run "replay.bat <file>".
rem Keys: 1 your camera, 2 your friend's, 3 free camera (Alt slow, Shift fast), 4 follow; Enter pause,
rem Left/Right seek 5 s (Shift 1 s), , . a frame while paused, Up/Down speed, Home start, H clean screen.
if "%~1"=="" (
  echo Drag a replay .bin file onto replay.bat, or run: replay.bat replays\ac1-replay-....bin
  pause
  exit /b 1
)
set AC1_REPLAY=%~f1
"%~dp0target\release\ac1.exe"
