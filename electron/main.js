const { app, BrowserWindow } = require("electron");

let win;
app.whenReady().then(() => {
    win = new BrowserWindow({
        height: 800,
        width: 1000,
        autoHideMenuBar: true,
        webPreferences: {
            nodeIntegration: true,
            contextIsolation: false,
            enableRemoteModule: true
        }
    });

    if (app.isPackaged) {
        win.loadFile("./gui/build/index.html");
    } else {
        win.loadURL("http://localhost:3000");
    }
});

app.on('window-all-closed', () => {
    if (process.platform !== 'darwin') {
        app.quit();
    }
});
