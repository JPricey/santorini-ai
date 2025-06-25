import os
import shlex
import subprocess
import time
import threading
import tkinter as tk
from tkinter import scrolledtext
import signal
from dataclasses import dataclass, field
import json
from frozendict import frozendict

BASIC_START_STRING = "0000000000000000000000000/1/mortal:B3,D3/mortal:C2,C4"

COL_LABEL_MAPPING = 'ABCDE'
ROW_LABEL_MAPPING = '12345'

INDEX_TO_COORD_MAPPING = []
COORD_TO_INDEX_MAPPING = dict()
for i in range(25):
    row = 4 - i // 5
    col = i % 5
    coord_str = f'{COL_LABEL_MAPPING[col]}{ROW_LABEL_MAPPING[row]}'

    COORD_TO_INDEX_MAPPING[coord_str] = i
    INDEX_TO_COORD_MAPPING.append(coord_str)


DONE_ACTION_TYPE = "DONE"


class EngineProcess:
    def __init__(self, output_callback):
        self.output_callback = output_callback
        self.process = None
        self.stdout_thread = None
        self.stderr_thread = None
        self.start_engine()

    def start_engine(self):
        start_command = shlex.split("cargo run -p uci --release")

        env = os.environ.copy()
        env['RUST_BACKTRACE'] = 'full'
        env['RUSTFLAGS'] = '-C target-cpu=native'

        self.process = subprocess.Popen(
            start_command,
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            bufsize=1,
            env=env,
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
                # print("[engine stdout]:", output.strip())
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

    if len(parts) != 4:
        print("Game state has wrong number of parts: ", len(parts))
        return None

    height_str = parts[0]
    if len(height_str) != 25:
        print('height string is wrong length: ', height_str, len(height_str))
        return None

    for i in range(5*5):
        h = height_str[i]
        if h.isdigit():
            height = int(h)
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
        worker_string = worker_string.replace('#', '')
        # TODO: parse the god
        worker_string_parts = worker_string.split(':')
        if len(worker_string_parts) != 2:
            print('worker string must have 2 parts')

        result = []
        for part in worker_string_parts[1].split(','):
            if part in COORD_TO_INDEX_MAPPING:
                res = COORD_TO_INDEX_MAPPING[part]
            else:
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


class BoardSquare(tk.Frame):
    def __init__(self, parent, index, click_callback=None, **kwargs):
        super().__init__(parent, bg="white", relief="ridge", borderwidth=2, **kwargs)

        self.index = index
        self.click_callback = click_callback

        # Index label (top-left)
        self.idx_label = tk.Label(
            self,
            text=str(index),
            bg="white",
            fg="black",
            font=("Arial", 8),
            anchor="nw"
        )
        self.idx_label.place(x=2, y=2)

        # Height label (large, centered, background)
        self.height_label = tk.Label(
            self,
            text="0",
            bg="white",
            fg="lightgray",
            font=("Arial", 36)
        )
        self.height_label.place(relx=0.3, rely=0.5, anchor="center")

        self.worker_label = tk.Label(
            self,
            text="",
            bg="white",
            font=("Arial", 24, "bold")
        )
        self.worker_label.place(relx=0.7, rely=0.5, anchor="center")

        self.bind("<Button-1>", self._on_click)
        for child in self.winfo_children():
            child.bind("<Button-1>", self._on_click)

    def _on_click(self, event):
        if self.click_callback:
            self.click_callback(self.index)

    def update_state(self, height=0, worker=None):
        self.height_label.config(text=str(height))

        if worker == 1:
            self.worker_label.config(text="X", fg="blue")
        elif worker == 2:
            self.worker_label.config(text="O", fg="red")
        else:
            self.worker_label.config(text="")


class GameBoardPanel(tk.Frame):
    def __init__(self, parent, width=400, height=400, click_callback=None, **kwargs):
        super().__init__(parent, **kwargs)
        self.width = width
        self.height = height
        self.click_callback = click_callback
        self.configure(bg="white", width=width, height=height)

        self.status_bar = tk.Entry(
            self,
            readonlybackground=self["background"],
            relief=tk.SUNKEN,
            bd=1
        )
        self.status_bar.insert(0, "Ready")
        self.status_bar.configure(state="readonly")
        self.status_bar.grid(row=0, column=0, columnspan=6, sticky="ew")

        self.buttons = []
        self.create_grid()
        self.set_position(BASIC_START_STRING)

    def create_grid(self):
        for i in range(5):
            row_label = tk.Label(self, text=str(5-i), width=2)
            row_label.grid(row=i+1, column=0, sticky="e")

        for j in range(5):
            col_label = tk.Label(self, text=chr(65+j))
            col_label.grid(row=6, column=j+1)

        self.buttons = []
        for i in range(5):
            for j in range(5):
                idx = i*5+j
                square = BoardSquare(
                    self, idx, click_callback=self.on_cell_click)
                square.grid(row=i+1, column=j+1, sticky="nsew", padx=2, pady=2)
                self.buttons.append(square)

        self.columnconfigure(0, weight=0)
        for j in range(1, 6):
            self.columnconfigure(j, weight=1, minsize=80, uniform="col")

        self.rowconfigure(0, weight=0)
        for i in range(1, 6):
            self.rowconfigure(i, weight=1, minsize=80, uniform="row")

    def on_cell_click(self, idx):
        if self.click_callback:
            self.click_callback(idx)

    def set_position(self, game_state_string):
        self.game_state_string = game_state_string
        self.game_state = parse_game_state(game_state_string)

        if self.game_state is None:
            new_text = f"Invalid: {self.game_state_string}"
        else:
            if self.game_state.player_1_turn:
                player_str = '1 (X)'
            else:
                player_str = '2 (O)'
            to_move = f'Player {player_str} to move.'
            new_text = f"{to_move} ({self.game_state_string})"

        self.status_bar.config(state="normal")
        self.status_bar.delete(0, tk.END)
        self.status_bar.insert(0, new_text)
        self.status_bar.config(state="readonly")

        self.draw_board()

    def draw_board(self):
        for idx in range(25):
            height = self.game_state.height_map[idx]
            worker = None
            if idx in self.game_state.player_1_workers:
                worker = 1
            elif idx in self.game_state.player_2_workers:
                worker = 2

            self.buttons[idx].update_state(height=height, worker=worker)


def pretty_string_for_action(action):
    action_type = action['type']

    if action_type == 'select_worker':
        return action['value']
    elif action_type == 'move_worker':
        return f'>{action["value"]}'
    elif action_type == 'build':
        return f'@{action["value"]}'
    elif action_type == DONE_ACTION_TYPE:
        return '<end>'

    print('ERROR: Unknown action type', action_type)


def longer_string_for_action(action):
    action_type = action['type']

    if action_type == 'select_worker':
        return f"Select {action['value']}"
    elif action_type == 'move_worker':
        return f"Move to {action['value']}"
    elif action_type == 'build':
        return f"Build at {action['value']}"
    elif action_type == DONE_ACTION_TYPE:
        return "End Turn"

    print('ERROR: Unknown action type', action_type)


def pretty_string_for_action_sequence(actions):
    return ' '.join(pretty_string_for_action(a) for a in actions)


@dataclass
class ActionSelector():
    current_position: str = None
    all_futures: list = None
    current_action_choices: list = None
    next_possible_actions: list = None

    def add_partial_action(self, partial_action):
        self.current_action_choices.append(partial_action)

    def get_all_possible_futures(self):
        result = []
        if len(self.current_action_choices) > 0 and self.current_action_choices[-1]['type'] == DONE_ACTION_TYPE:
            for future in self.all_futures:
                if future['actions'] == self.current_action_choices[:-1]:
                    result.append(future)
        else:
            for future in self.all_futures:
                if self.current_action_choices == future['actions'][:len(self.current_action_choices)]:
                    result.append(future)

        return result

    def update_next_possible_actions(self):
        result = set()
        possible_futures = self.get_all_possible_futures()

        for future in possible_futures:
            if len(future['actions']) == len(self.current_action_choices):
                result.add(frozendict(type=DONE_ACTION_TYPE))
            elif len(future['actions']) > len(self.current_action_choices):
                next_action = future['actions'][len(
                    self.current_action_choices)]
                result.add(frozendict(next_action))

        self.next_possible_actions = list(
            sorted(result, key=pretty_string_for_action))


class PositionHistory:
    def __init__(self, initial_position_string):
        self.positions_history = [initial_position_string]
        self.current_index = 0

    def current_position_string(self):
        return self.positions_history[self.current_index]

    def undo(self):
        if self.current_index > 0:
            self.current_index -= 1
            return True
        return False

    def redo(self):
        if self.current_index < len(self.positions_history) - 1:
            self.current_index += 1
            return True
        return False

    def add_next_position(self, position_string):
        if self.current_index < len(self.positions_history) - 1:
            self.positions_history = self.positions_history[:self.current_index + 1]
        self.positions_history.append(position_string)
        self.current_index += 1


class RootPanel:
    def __init__(self, root):
        self.root = root
        self.root.title("Game Analysis Engine")
        self.root.geometry("1620x960")

        self.action_selector = None
        self.last_engine_move = None
        self.position_history = PositionHistory(BASIC_START_STRING)

        self.root.bind("<Control-w>", lambda event: self.on_closing())
        self.root.bind("<Up>", lambda event: self.on_up_key())
        self.root.bind("<Left>", lambda event: self.on_left_key())
        self.root.bind("<Right>", lambda event: self.on_right_key())

        self.analysis_results = []

        self.create_ui()
        self.engine = EngineProcess(self.handle_engine_output)

        self.root.protocol("WM_DELETE_WINDOW", self.on_closing)

        self.position_to_action_cache = {}

    def current_position_string(self):
        return self.position_history.current_position_string()

    def on_up_key(self):
        if self.last_engine_move is None or self.last_engine_move['start_state'] != self.current_position_string():
            print('tried to pick engine move but it was invalid')
            return
        self.update_position(self.last_engine_move['next_state'])

    def on_left_key(self):
        if self.position_history.undo():
            self.on_position_updated()

    def on_right_key(self):
        if self.position_history.redo():
            self.on_position_updated()

    def create_ui(self):
        # Configure grid layout
        self.root.columnconfigure(0, weight=3)  # Game board gets more space
        self.root.columnconfigure(1, weight=1)  # Right side gets less space
        self.root.rowconfigure(0, weight=1)     # Main content area expands
        self.root.rowconfigure(1, weight=0)     # Bottom controls don't expand

        # Create the game board panel (left side)
        self.game_board = GameBoardPanel(
            self.root, width=500, height=500, click_callback=self.on_click_cell)
        self.game_board.grid(row=0, column=0, padx=10, pady=10, sticky="nsew")

        # Create right-side frame to contain both text areas
        right_frame = tk.Frame(self.root)
        right_frame.grid(row=0, column=1, padx=10, pady=10, sticky="nsew")
        right_frame.rowconfigure(0, weight=1)  # Analysis area
        right_frame.rowconfigure(1, weight=1)  # Output area
        right_frame.columnconfigure(0, weight=1)

        action_frame = tk.Frame(right_frame)
        action_frame.grid(row=0, column=0, pady=(0, 5), sticky="nsew")
        action_frame.rowconfigure(0, weight=1)
        action_frame.columnconfigure(0, weight=1)
        self.action_options_panel = tk.Listbox(
            action_frame, width=30, height=10)
        self.action_options_panel.grid(row=0, column=0, sticky="nsew")
        self.action_options_panel.bind(
            '<<ListboxSelect>>', self.on_action_selected)

        # Output area (bottom of right side)
        self.engine_output = scrolledtext.ScrolledText(
            right_frame, wrap=tk.WORD, width=30, height=20)
        self.engine_output.grid(row=1, column=0, pady=(5, 0), sticky="nsew")

        # Control frame at the bottom
        control_frame = tk.Frame(self.root)
        control_frame.grid(row=1, column=0, columnspan=2,
                           padx=10, pady=5, sticky="ew")

        self.input_field = tk.Entry(control_frame, width=60)
        self.input_field.grid(row=0, column=0, padx=5, pady=5, sticky="ew")
        self.input_field.insert(0, BASIC_START_STRING)

        def select_all_text(event):
            event.widget.select_range(0, tk.END)
            event.widget.icursor(tk.END)  # Move cursor to the end
            return "break"  # Prevent default behavior
        self.input_field.bind(
            "<Control-a>", lambda event: select_all_text(event))

        set_pos_button = tk.Button(
            control_frame, text="Set Position", command=self.pressed_set_position)
        set_pos_button.grid(row=0, column=1, padx=5, pady=5)

    def on_click_cell(self, idx):
        coord = INDEX_TO_COORD_MAPPING[idx]

        if self.action_selector is None or self.action_selector.current_position != self.current_position_string():
            print('Actions out of sync')
            return

        possibly_pressed_actions = []
        for action in self.action_selector.next_possible_actions:
            if action.get('value') == coord:
                possibly_pressed_actions.append(action)

        if len(possibly_pressed_actions) == 1:
            self.action_selector.add_partial_action(
                possibly_pressed_actions[0])
            self.check_for_completed_action()
        elif len(possibly_pressed_actions) > 1:
            print('duplicate possible actions??', possibly_pressed_actions)

    def pressed_set_position(self):
        input_field = self.input_field.get()
        self.update_position(input_field)

    def update_position(self, position_string):
        position_string = position_string.strip()

        if position_string:
            self.position_history.add_next_position(position_string)
            self.on_position_updated()

    def on_position_updated(self):
        self.engine_output.delete("1.0", tk.END)
        self.engine.send_command(
            f'set_position {self.current_position_string()}')
        self.game_board.set_position(self.current_position_string())
        self.action_selector = None
        self.try_start_action_sequence()

    def handle_engine_output(self, message):
        self.root.after(0, lambda: self.handle_message(message))

    def handle_message(self, raw_message):
        try:
            message = json.loads(raw_message)
        except json.JSONDecodeError as e:
            print(f"JSON parsing error: {e}")
            return None

        message_type = message['type']
        if message_type == 'best_move':
            self.handle_best_move_message(message)
        elif message_type == 'next_moves':
            self.handle_next_moves_message(message)
        elif message_type == 'started':
            self.handle_started_message(message)
        else:
            print('Unknown message type', message_type, raw_message)

    def try_start_action_sequence(self):
        next_moves = self.position_to_action_cache.get(
            self.current_position_string())
        if next_moves is None:
            self.engine.send_command(
                f'next_moves {self.current_position_string()}')
            return

        if self.action_selector is None or self.action_selector.current_position != self.current_position_string():
            self.action_selector = ActionSelector(
                current_position=self.current_position_string(),
                all_futures=next_moves['next_states'],
                current_action_choices=[]
            )
        else:
            print('duplicate setting of action sequence?')

        self.set_action_sequence_options()

    def set_action_sequence_options(self):
        self.action_options_panel.delete(0, tk.END)
        self.action_selector.update_next_possible_actions()
        next_options = self.action_selector.next_possible_actions
        for option in next_options:
            stringed = longer_string_for_action(option)
            self.action_options_panel.insert(tk.END, stringed)

    def on_action_selected(self, event):
        selected_indices = self.action_options_panel.curselection()
        if selected_indices:
            index = selected_indices[0]
            # value = self.action_options_panel.get(index)
            data = self.action_selector.next_possible_actions[index]
            self.action_selector.add_partial_action(data)
            self.check_for_completed_action()

    def check_for_completed_action(self):
        all_possible_futures = self.action_selector.get_all_possible_futures()
        if len(all_possible_futures) == 1:
            future = all_possible_futures[0]
            future_state = future['next_state']
            self.update_position(future_state)
        else:
            self.set_action_sequence_options()

    def handle_next_moves_message(self, message):
        start_state = message['start_state']
        self.position_to_action_cache[start_state] = message
        self.try_start_action_sequence()

    def handle_best_move_message(self, message):
        if message['start_state'] != self.current_position_string():
            print('Skipping best move for non-current position')
            return

        self.last_engine_move = message

        trigger = message['trigger']
        meta = message['meta']
        action_string = pretty_string_for_action_sequence(meta['actions'])

        thinking_string = f"{action_string} (eval: {meta['score']}) ({meta['elapsed_seconds']:.2f}s | depth {meta['calculated_depth']}) | {trigger}\n"

        self.engine_output.insert('1.0', thinking_string)
        self.engine_output.see('1.0')

    def handle_started_message(self, message):
        self.update_position(self.current_position_string())

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
