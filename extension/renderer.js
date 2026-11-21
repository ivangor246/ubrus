import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import Meta from 'gi://Meta';

export class Renderer {
    constructor(scene, notice, exited) {
        const launcher = new Gio.SubprocessLauncher({
            flags: Gio.SubprocessFlags.STDIN_PIPE | Gio.SubprocessFlags.STDOUT_PIPE,
        });
        let executable = scene.snap ? '/snap/bin/ubrus' : '/usr/bin/ubrus';
        if (
            !scene.snap &&
            typeof scene.native_launcher === 'string' &&
            /^ubrus-native-[A-Za-z0-9]+$/.test(scene.native_launcher)
        )
            executable = GLib.build_filenamev([GLib.get_user_runtime_dir(), scene.native_launcher, 'ubrus']);
        const argv = [GLib.getenv('UBRUS_RENDERER') || executable, '--renderer'];
        this.cancel = new Gio.Cancellable();
        try {
            // Mutter 50 supports a Wayland compositor exclusively.
            if (!Meta.is_wayland_compositor || Meta.is_wayland_compositor()) {
                if (Meta.WaylandClient.new_subprocess) {
                    this.client = Meta.WaylandClient.new_subprocess(global.context, launcher, argv);
                    this.proc = this.client.get_subprocess();
                } else {
                    this.client = Meta.WaylandClient.new(global.context, launcher);
                    this.proc = this.client.spawnv(global.display, argv);
                }
            } else {
                this.proc = launcher.spawnv(argv);
            }
        } finally {
            launcher.close();
        }
        console.info(`[Ubrus] Started renderer ${this.proc.get_identifier()}`);
        this.input = this.proc.get_stdin_pipe();
        const output = new Gio.DataInputStream({ base_stream: this.proc.get_stdout_pipe() });
        const read = () =>
            output.read_line_async(GLib.PRIORITY_DEFAULT, this.cancel, (stream, res) => {
                if (this.cancel.is_cancelled()) return;
                try {
                    const [line] = stream.read_line_finish_utf8(res);
                    if (line === null) return;
                    if (line.startsWith('UBRUS/1 ')) notice(JSON.parse(line.slice(8)));
                    read();
                } catch (e) {
                    notice({ error: String(e) });
                }
            });
        read();
        this.proc.wait_async(null, (p, res) => {
            p.wait_finish(res);
            if (!this.cancel.is_cancelled()) exited();
        });
    }

    owns(window) {
        return this.client
            ? this.client.owns_window(window)
            : window.get_pid() === Number(this.proc.get_identifier());
    }

    background(window) {
        if (this.client) {
            if (window.set_type) {
                window.set_type(Meta.WindowType.DESKTOP);
                window.hide_from_window_list();
            } else {
                this.client.make_desktop(window);
                this.client.hide_from_window_list(window);
            }
        }
        window.stick();
    }

    send(screens, serial) {
        this.pending = JSON.stringify({ screens, serial });
        this.flush();
    }

    inspect() {
        this.wantStatus = true;
        this.flush();
    }

    flush() {
        if (this.writing || this.cancel.is_cancelled()) return;
        const message = this.pending ?? (this.wantStatus ? 'status' : null);
        if (message === null) return;
        if (this.pending != null) this.pending = null;
        else this.wantStatus = false;
        this.writing = true;
        const bytes = new TextEncoder().encode(message + '\n');
        this.input.write_all_async(bytes, GLib.PRIORITY_DEFAULT, this.cancel, (s, res) => {
            this.writing = false;
            if (this.cancel.is_cancelled()) return;
            try {
                s.write_all_finish(res);
                this.flush();
            } catch {
                /* wait_async handles the process exit */
            }
        });
    }

    close() {
        this.cancel.cancel();
        const pid = this.proc.get_identifier();
        if (pid !== null) {
            console.info(`[Ubrus] Stopping renderer ${pid}`);
            this.proc.force_exit();
        }
    }
}
