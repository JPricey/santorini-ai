import subprocess
import os
import time

# rsync -chavzP -e "ssh -p $REMOTE_PORT" root@$REMOTE_HOST:/root/santorini/game_data/* tmp/gen_3/

DEST_PATH = 'tmp/gen_3/new_gods_batch_3'

def read_remotes_file():
    remotes = []
    with open("tmp/remotes.txt", "r") as f:
        for line in f:
            line = line.strip()
            if line and not line.startswith("#"):
                ip, port = line.split(":")
                remotes.append((ip, port))
    return remotes

def count_total_lines_in_dir(dir_path):
    total = 0
    for fname in os.listdir(dir_path):
        fpath = os.path.join(dir_path, fname)
        if os.path.isfile(fpath):
            with open(fpath, 'r', encoding='utf-8', errors='ignore') as f:
                total += sum(1 for _ in f)
    return total

def pull(ip, port):
    start_time = time.strftime('%Y-%m-%d %H:%M:%S')
    print(f"[{start_time}] Trying to pull from {ip}:{port}")
    remote_path = "/root/santorini/game_data/*"
    local_path = DEST_PATH

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
    remotes = read_remotes_file()

    for (ip, port) in remotes:
        pull(ip, port)

    end_time = time.strftime('%Y-%m-%d %H:%M:%S')
    print(f"[{end_time}] Total lines:", count_total_lines_in_dir(DEST_PATH))

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

