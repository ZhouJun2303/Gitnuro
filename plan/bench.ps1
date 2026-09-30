# AweGit refresh benchmark. Do not run until the large clones exist.
# Scenarios (record three times: before P0, after P0, after P1/P2):
# 1. Cold open until the first log screen
# 2. Change 1 file until the status pane updates
# 3. Type in the commit box for 5 seconds and count full refreshes
# 4. Scroll 10,000 log rows
# 5. Memory after opening the repository
# 6. Run a build inside the repository and count status refreshes
#
# Clone once, manually:
#   git clone https://github.com/microsoft/vscode D:\bench\vscode
#   git clone https://github.com/git/git D:\bench\git
#
# Timings are printed by measureAndLog to the AweGit log.
Write-Output "Benchmark script is ready. It does not clone repositories or launch AweGit."
