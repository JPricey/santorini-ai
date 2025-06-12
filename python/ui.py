import shlex
import subprocess
import sys
import time
import threading

start_command = shlex.split("cargo run --bin uci --release")

set_position_command = 'set_position 2111202211011420002000100/1/7,14/2,18'

process = subprocess.Popen(
    start_command,
    stdin=subprocess.PIPE,
    stdout=subprocess.PIPE,
    stderr=subprocess.PIPE,
    text=True,
    bufsize=1
)


def handle_message(source, message):
    print(f"[{source}] {message}")


def listen_to_stdout() -> None:
    while process.poll() is None:
        print('stdout listen')
        output = process.stdout.readline()
        if output:
            handle_message("stdout", output.strip())
        else:
            time.sleep(0.1)

# Thread function to continuously read stderr


def listen_to_stderr() -> None:
    while process.poll() is None:
        print('stderr listen')
        output = process.stderr.readline()
        if output:
            handle_message("stderr", output.strip())
        else:
            time.sleep(0.1)


# Start the listener threads
stdout_thread = threading.Thread(target=listen_to_stdout, daemon=True)
stderr_thread = threading.Thread(target=listen_to_stderr, daemon=True)
stdout_thread.start()
stderr_thread.start()


while process.poll() is None:
    try:
        human_input = input('> ')

        process.stdin.write(f"{human_input}\n")
        process.stdin.flush()
    except KeyboardInterrupt:
        break
