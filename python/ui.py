import subprocess
import os

current_directory = os.getcwd()
print(current_directory)

cmd = 'cargo run --bin uci --release'.split(' ')
print('cmd: ', cmd)
subprocess.run(cmd)
