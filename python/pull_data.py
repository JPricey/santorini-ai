import subprocess
import os
import time

REMOTES = [
    ("1.208.108.242", "54427"),
    ("216.209.229.43", "35641"),
    ("171.235.166.92", "42291"),
    ("182.227.40.157", "50764"),
    ("115.72.153.141", "10143"),
]

# rsync -chavzP -e "ssh -p $REMOTE_PORT" root@$REMOTE_HOST:/root/santorini/game_data/* tmp/gen_3/

def pull(ip, port):
    print(f"Trying to pull from {ip}:{port}")
    remote_path = "/root/santorini/game_data/*"
    local_path = "tmp/gen_3/"
    
    os.makedirs(local_path, exist_ok=True)

    command = [
        "rsync", "-chavzP",
        "-e", f"ssh -p {port}",
        f"root@{ip}:{remote_path}",
        local_path
    ]

    try:
        subprocess.run(command, check=True)
        print(f"Data pulled successfully from {ip}:{port}")
    except subprocess.CalledProcessError as e:
        print(f"Error pulling data from {ip}:{port}: {e}")

def pull_all_once():
    for (ip, port) in REMOTES:
        pull(ip, port)

def main():
    # pull_all_once()
    pull_in_loop()

def pull_in_loop():
    while True:
        pull_all_once()
        print("Done pulling, waiting...")
        time.sleep(60 * 5)


if __name__ == "__main__":
    main()

