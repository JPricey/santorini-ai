import os
import shlex
import subprocess
import time
import threading
import tkinter as tk
from tkinter import scrolledtext, messagebox
import signal
from dataclasses import dataclass, field, asdict


class EngineProcess:
    def __init__(self, output_callback):
        self.output_callback = output_callback
        self.process = None
        self.stdout_thread = None
        self.stderr_thread = None
        self.start_engine()

    def start_engine(self):
        start_command = shlex.split("cargo run --bin uci --release")

        self.process = subprocess.Popen(
            start_command,
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            bufsize=1
        )

        self.start_threads()

    def send_command(self, command):
        self.process.stdin.write(f"{command}\n")
        self.process.stdin.flush()

    def start_threads(self):
        self.stdout_thread = threading.Thread(
            target=self.listen_to_stdout, daemon=True)
        self.stderr_thread = threading.Thread(
            target=self.listen_to_stderr, daemon=True)
        self.stdout_thread.start()
        self.stderr_thread.start()

    def listen_to_stdout(self):
        while self.process.poll() is None:
            output = self.process.stdout.readline()
            if output:
                print("[engine stdout]:", output)
                self.output_callback(output.strip())
            else:
                time.sleep(0.1)

    def listen_to_stderr(self):
        while self.process.poll() is None:
            output = self.process.stderr.readline()
            if output:
                print("[engine stderr]:", output.strip())
            else:
                time.sleep(0.1)


def parse_game_state(game_state_string):
    result = GameState()
    game_state_string = ''.join(game_state_string.split())
    parts = game_state_string.split('/')
    for part in parts:
        print('part', part)

    if len(parts) != 4:
        print("Game state has wrong number of parts: ", len(parts))
        return None

    height_str = parts[0]
    if len(height_str) != 25:
        print('height string is wrong length: ', height_str, len(height_str))
        return None

    idx = 0
    for i in range(5*5):
        h = height_str[i]
        if h.isdigit():
            height = int(height_str[idx])
            if height >= 0 and height <= 4:
                result.height_map[i] = height
                continue
        print(f'Height at {i} is invalid: {h}')
        return None

    turn_str = parts[1]
    if turn_str not in ['1', '2']:
        print("Turn str: ", turn_str)
        return None

    result.player_1_turn = turn_str == "1"

    def parse_worker_string(worker_string):
        result = []
        for part in worker_string.split(','):
            res = int(part)
            if res >= 0 and res < 25:
                result.append(res)
            else:
                return None
        return result

    result.player_1_workers = parse_worker_string(parts[2])
    result.player_2_workers = parse_worker_string(parts[3])

    if result.player_1_workers is None or result.player_2_workers is None:
        return None

    return result


@dataclass
class GameState:
    height_map: list = field(default_factory=lambda: [0 for _ in range(25)])
    player_1_workers: list = field(default_factory=list)
    player_2_workers: list = field(default_factory=list)
    player_1_turn: bool = True


class GameBoardPanel(tk.Frame):
    def __init__(self, parent, width=400, height=400, **kwargs):
        super().__init__(parent, **kwargs)
        self.width = width
        self.height = height
        self.configure(bg="white", width=width, height=height)

        self.status_bar = tk.Label(
            self, text="Ready", bd=1, relief=tk.SUNKEN, anchor=tk.W)
        self.status_bar.grid(row=0, column=0, columnspan=5, sticky="ew")

        self.buttons = []
        self.create_grid()
        self.set_position("0000000000000000000000000/1/11,13/7,17")

    def create_grid(self):
        for i in range(5):
            row_label = tk.Label(self, text=str(5-i), width=2)
            row_label.grid(row=i+1, column=0, sticky="e")

        for j in range(5):
            col_label = tk.Label(self, text=chr(65+j))
            col_label.grid(row=6, column=j+1)

        for i in range(5):
            for j in range(5):
                txt = str(len(self.buttons))
                btn = tk.Button(
                    self,
                    width=5,
                    height=2,
                    text=txt,
                    bg="white",
                    relief="ridge",
                    borderwidth=2
                )
                btn.grid(row=i+1, column=j+1, sticky="nsew", padx=2, pady=2)
                self.buttons.append(btn)

                self.columnconfigure(j+1, weight=1)
            self.rowconfigure(i + 1, weight=1)

        self.columnconfigure(0, weight=0)
        self.rowconfigure(0, weight=0)

    def set_position(self, game_state_string):
        self.game_state_string = game_state_string
        self.game_state = parse_game_state(game_state_string)

        if self.game_state is None:
            self.status_bar.config(text=f"Invalid: {self.game_state_string}")
        else:
            self.status_bar.config(text=f"Position: {self.game_state_string}")

        self.draw_board()

    def draw_board(self):
        print('draw board', self.game_state_string)


class RootPanel:
    def __init__(self, root):
        self.root = root
        self.root.title("Game Analysis Engine")
        self.root.geometry("800x600")

        self.current_position = None
        self.analysis_results = []

        self.create_ui()
        self.engine = EngineProcess(self.handle_engine_output)

        self.root.protocol("WM_DELETE_WINDOW", self.on_closing)

    def create_ui(self):
        self.root.columnconfigure(0, weight=3)  # Game board gets more space
        self.root.columnconfigure(1, weight=1)
        self.root.rowconfigure(0, weight=1)     # Main content area expands
        self.root.rowconfigure(1, weight=0)     # Bottom controls don't expand

        self.game_board = GameBoardPanel(self.root, width=500, height=500)
        self.game_board.grid(row=0, column=0, padx=10, pady=10, sticky="nsew")

        self.output_area = scrolledtext.ScrolledText(
            self.root, wrap=tk.WORD, width=30, height=20)
        self.output_area.grid(row=0, column=1, padx=10, pady=10, sticky="nsew")

        control_frame = tk.Frame(self.root)
        control_frame.grid(row=1, column=0, columnspan=2,
                           padx=10, pady=5, sticky="ew")

        control_frame.columnconfigure(0, weight=1)  # Input field expands

        self.input_field = tk.Entry(control_frame, width=60)
        self.input_field.grid(row=0, column=0, padx=5, pady=5, sticky="ew")

        set_pos_button = tk.Button(
            control_frame, text="Set Position", command=self.pressed_set_position)
        set_pos_button.grid(row=0, column=2, padx=5, pady=5)

    def pressed_set_position(self):
        input_field = self.input_field.get()
        self.update_position(input_field)

    def update_position(self, position_string):
        position_string = position_string.strip()
        print('update position called', position_string)
        if position_string:
            self.current_position = position_string
            self.game_board.set_position(position_string)
            self.engine.send_command(f'set_position {position_string}')
        # self.input_field.index

    def handle_engine_output(self, message):
        self.root.after(0, lambda: self.handle_message(message))

    def handle_message(self, message):
        self.output_area.insert(tk.END, f"{message}\n")
        self.output_area.see(tk.END)

    def check_exit_flag(self):
        self.root.after(100, self.check_exit_flag)

    def on_closing(self):
        self.root.destroy()


if __name__ == "__main__":
    root = tk.Tk()
    app = RootPanel(root)

    def signal_handler(sig, frame):
        print("\nCTRL+C detected. Shutting down...")
        app.on_closing()  # Properly close the application

    signal.signal(signal.SIGINT, signal_handler)

    app.check_exit_flag()

    root.mainloop()
