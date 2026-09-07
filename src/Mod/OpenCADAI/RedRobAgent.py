# SPDX-License-Identifier: LGPL-2.1-or-later
"""Native Qt client for the local RedRob Code engine.

RedRob Code owns authentication, model access, sessions, and agent execution. This
module talks to its documented local service through ``redrob api`` so OpenCAD
never stores workspace keys or reimplements the engine protocol.
"""

from __future__ import annotations

import html
import json
import os
import re
from pathlib import Path

import FreeCAD as App
import FreeCADGui as Gui
from PySide import QtCore, QtWidgets

_PARAM_PATH = "User parameter:BaseApp/Preferences/OpenCADAI"
_DOCK_NAME = "OpenCADRedRobAgent"
_panel = None


def _parameters():
    return App.ParamGet(_PARAM_PATH)


def _working_directory():
    configured = _parameters().GetString("WorkingDirectory", "").strip()
    if configured:
        return os.path.abspath(os.path.expanduser(configured))
    document = App.ActiveDocument
    if document and document.FileName:
        return os.path.dirname(os.path.abspath(document.FileName))
    return str(Path.home())


def _document_context():
    document = App.ActiveDocument
    if document is None:
        return "No CAD document is currently open."

    objects = []
    for obj in document.Objects[:250]:
        objects.append(
            {
                "name": obj.Name,
                "label": obj.Label,
                "type": obj.TypeId,
                "visibility": getattr(getattr(obj, "ViewObject", None), "Visibility", None),
            }
        )

    selection = []
    for item in Gui.Selection.getSelectionEx():
        selection.append(
            {
                "object": item.ObjectName,
                "subelements": list(item.SubElementNames),
            }
        )

    return json.dumps(
        {
            "document": {
                "name": document.Name,
                "label": document.Label,
                "file": document.FileName or None,
                "object_count": len(document.Objects),
            },
            "selection": selection,
            "objects": objects,
        },
        indent=2,
        ensure_ascii=False,
    )


def _decode_json(output):
    """Decode JSON even when a CLI version writes a short status prefix."""
    decoder = json.JSONDecoder()
    for marker in ("{", "["):
        offset = output.find(marker)
        while offset >= 0:
            try:
                value, _ = decoder.raw_decode(output[offset:])
                return value
            except json.JSONDecodeError:
                offset = output.find(marker, offset + 1)
    raise ValueError("RedRob returned no JSON payload")


def _unwrap_data(value):
    while isinstance(value, dict) and set(value).issuperset({"data"}):
        value = value["data"]
    return value


def _redrob_error(value):
    """Return a readable API error because ``redrob api`` can exit zero on HTTP errors."""
    value = _unwrap_data(value)
    if not isinstance(value, dict):
        return ""
    tag = str(value.get("_tag", ""))
    error = value.get("error")
    if tag.endswith("Error") or error:
        return str(value.get("message") or error or tag or value)
    return ""


def _assistant_text(context):
    context = _unwrap_data(context)
    if not isinstance(context, list):
        raise TypeError("RedRob returned a non-list session context")

    replies = []
    assistant_seen = False
    for message in context:
        if not isinstance(message, dict):
            raise TypeError("RedRob returned a malformed context message")
        if message.get("type") != "assistant":
            continue
        assistant_seen = True
        content = message.get("content")
        if not isinstance(content, list):
            raise TypeError("RedRob returned malformed assistant content")
        parts = []
        for part in content:
            if not isinstance(part, dict):
                raise TypeError("RedRob returned a malformed assistant content part")
            if part.get("type") == "text":
                text = part.get("text")
                if not isinstance(text, str):
                    raise ValueError("RedRob returned non-text assistant content")
                if text:
                    parts.append(text)
        if parts:
            replies.append("\n".join(parts))
    if not assistant_seen:
        raise ValueError("RedRob context did not contain an assistant response")
    return replies[-1] if replies else "The agent completed without a textual response."


class RedRobPanel(QtWidgets.QWidget):
    """Dockable RedRob conversation and reviewed CAD-script runner."""

    def __init__(self, parent=None):
        super().__init__(parent)
        self._process = None
        self._interrupt_process = None
        self._session_id = None
        self._session_binding = None
        self._pending_binding = None
        self._request_directory = None
        self._operation_id = 0
        self._last_response = ""
        self._build_ui()

    def _build_ui(self):
        layout = QtWidgets.QVBoxLayout(self)
        layout.setContentsMargins(10, 10, 10, 10)
        layout.setSpacing(8)

        title_row = QtWidgets.QHBoxLayout()
        title = QtWidgets.QLabel("<b>RedRob Code</b>")
        title_row.addWidget(title)
        title_row.addStretch(1)
        self.agent = QtWidgets.QComboBox()
        self.agent.addItems(["plan", "build"])
        self.agent.setToolTip("Plan denies edit tools; build follows RedRob permission policy.")
        title_row.addWidget(self.agent)
        settings = QtWidgets.QToolButton()
        settings.setText("Settings")
        settings.clicked.connect(self.configure)
        title_row.addWidget(settings)
        layout.addLayout(title_row)

        self.transcript = QtWidgets.QTextBrowser()
        self.transcript.setOpenExternalLinks(True)
        self.transcript.setPlaceholderText(
            "Ask RedRob to inspect the active document, plan a model, or generate FreeCAD Python."
        )
        layout.addWidget(self.transcript, 1)

        self.prompt = QtWidgets.QPlainTextEdit()
        self.prompt.setPlaceholderText("Describe the CAD task… (Ctrl+Enter to send)")
        self.prompt.setMaximumHeight(120)
        self.prompt.installEventFilter(self)
        layout.addWidget(self.prompt)

        options = QtWidgets.QHBoxLayout()
        self.include_context = QtWidgets.QCheckBox("Include active document context")
        self.include_context.setChecked(True)
        options.addWidget(self.include_context)
        options.addStretch(1)
        self.run_code_button = QtWidgets.QPushButton("Run reviewed Python")
        self.run_code_button.setEnabled(False)
        self.run_code_button.clicked.connect(self.run_python)
        options.addWidget(self.run_code_button)
        self.stop_button = QtWidgets.QPushButton("Stop")
        self.stop_button.setEnabled(False)
        self.stop_button.clicked.connect(self.stop)
        options.addWidget(self.stop_button)
        self.send_button = QtWidgets.QPushButton("Send")
        self.send_button.clicked.connect(self.send)
        options.addWidget(self.send_button)
        layout.addLayout(options)

        self.status = QtWidgets.QLabel(
            "Uses your local RedRob Code service; credentials remain managed by RedRob."
        )
        self.status.setWordWrap(True)
        layout.addWidget(self.status)

    def eventFilter(self, watched, event):
        if (
            watched is self.prompt
            and event.type() == QtCore.QEvent.KeyPress
            and event.key() in (QtCore.Qt.Key_Return, QtCore.Qt.Key_Enter)
            and event.modifiers() & QtCore.Qt.ControlModifier
        ):
            self.send()
            return True
        return super().eventFilter(watched, event)

    def _set_busy(self, busy, message=""):
        self.send_button.setEnabled(not busy)
        self.stop_button.setEnabled(busy)
        self.agent.setEnabled(not busy)
        if message:
            self.status.setText(message)

    def _append(self, speaker, text):
        safe = html.escape(text).replace("\n", "<br>")
        self.transcript.append(f"<p><b>{html.escape(speaker)}</b><br>{safe}</p>")

    def _binary(self):
        return _parameters().GetString("Executable", "redrob").strip() or "redrob"

    def _run(self, arguments, callback, working_directory=None, operation_id=None):
        if self._process is not None:
            self._process.deleteLater()
        process = QtCore.QProcess(self)
        request_id = self._operation_id if operation_id is None else operation_id
        self._process = process
        process.setProgram(self._binary())
        process.setArguments(arguments)
        process.setWorkingDirectory(working_directory or self._request_directory or _working_directory())

        def finished(exit_code, _exit_status):
            if request_id != self._operation_id:
                process.deleteLater()
                return
            stdout = bytes(process.readAllStandardOutput()).decode("utf-8", "replace")
            stderr = bytes(process.readAllStandardError()).decode("utf-8", "replace")
            callback(exit_code, stdout, stderr)

        process.finished.connect(finished)
        process.errorOccurred.connect(lambda error: self._process_error(error, request_id))
        process.start()

    def _process_error(self, error, request_id):
        if request_id != self._operation_id:
            return
        if error == QtCore.QProcess.FailedToStart:
            self._set_busy(False, "RedRob executable was not found. Open Settings to configure it.")
            self._append(
                "OpenCAD",
                "Install RedRob Code from https://console.redrob.ai/code, then authenticate with "
                "`redrob providers login --provider redrob`.",
            )

    def _api(self, method, path, payload, callback):
        arguments = [
            "api",
            method,
            path,
            "--header",
            "x-redrob-directory:" + (self._request_directory or _working_directory()),
        ]
        if payload is not None:
            arguments.extend(["--data", json.dumps(payload, ensure_ascii=False)])
        self._run(arguments, callback)

    def send(self):
        prompt = self.prompt.toPlainText().strip()
        if not prompt or self._process_running():
            return
        self._operation_id += 1
        self._request_directory = _working_directory()
        model = _parameters().GetString("Model", "redrob/auto").strip()
        self._pending_binding = (self.agent.currentText(), model, self._request_directory)
        if self._session_binding != self._pending_binding:
            self._session_id = None
        self.prompt.clear()
        self._append("You", prompt)
        self._set_busy(True, "Starting the local RedRob Code service…")
        self._pending_prompt = prompt
        self._run(["service", "start"], self._after_service_start)

    def _after_service_start(self, exit_code, stdout, stderr):
        if exit_code != 0:
            self._fail(
                "This RedRob Code build does not expose the required local API service",
                stderr or stdout,
            )
            return
        if self._session_id:
            self._submit_prompt()
            return
        self.status.setText("Creating a RedRob session…")
        agent, model, directory = self._pending_binding
        payload = {
            "agent": agent,
            "location": {"directory": directory},
        }
        if model:
            payload["model"] = {"providerID": "redrob", "id": model}
        self._api("POST", "/api/session", payload, self._after_session_created)

    def _after_session_created(self, exit_code, stdout, stderr):
        if exit_code != 0:
            self._fail("Could not create a RedRob session", stderr or stdout)
            return
        try:
            session = _unwrap_data(_decode_json(stdout))
            api_error = _redrob_error(session)
            if api_error:
                raise ValueError(api_error)
            self._session_id = session["id"]
            self._session_binding = self._pending_binding
        except (ValueError, KeyError, TypeError) as error:
            self._fail("Could not read the RedRob session response", str(error) + "\n" + stdout)
            return
        self._submit_prompt()

    def _submit_prompt(self):
        self.status.setText("RedRob is reasoning…")
        text = self._pending_prompt
        if self.include_context.isChecked():
            text = (
                "You are assisting inside OpenCAD, a FreeCAD-compatible CAD application. "
                "Use the supplied document snapshot. For model-changing tasks, return one "
                "complete FreeCAD Python script in a ```python fenced block; do not manage "
                "document transactions and do not claim the script has run. The user reviews "
                "and executes it in OpenCAD.\n\n"
                "ACTIVE DOCUMENT SNAPSHOT\n"
                + _document_context()
                + "\n\nUSER REQUEST\n"
                + text
            )
        path = f"/api/session/{self._session_id}/prompt"
        self._api("POST", path, {"prompt": {"text": text}}, self._after_prompt_admitted)

    def _after_prompt_admitted(self, exit_code, stdout, stderr):
        if exit_code != 0:
            self._fail("RedRob rejected the prompt", stderr or stdout)
            return
        try:
            admission = _unwrap_data(_decode_json(stdout))
            api_error = _redrob_error(admission)
            if api_error:
                raise ValueError(api_error)
            if (
                not isinstance(admission, dict)
                or not isinstance(admission.get("id"), str)
                or not admission["id"]
                or not isinstance(admission.get("sessionID"), str)
                or admission["sessionID"] != self._session_id
                or isinstance(admission.get("admittedSeq"), bool)
                or not isinstance(admission.get("admittedSeq"), int)
                or admission["admittedSeq"] < 0
            ):
                raise ValueError("RedRob returned an invalid prompt-admission payload")
        except (ValueError, TypeError) as error:
            self._fail("RedRob rejected the prompt", str(error) + "\n" + stdout)
            return
        self.status.setText("Waiting for RedRob to finish…")
        self._api(
            "POST",
            f"/api/session/{self._session_id}/wait",
            None,
            self._after_wait,
        )

    def _after_wait(self, exit_code, stdout, stderr):
        if exit_code != 0:
            self._fail("RedRob did not complete the request", stderr or stdout)
            return
        if stdout.strip():
            try:
                wait_payload = _decode_json(stdout)
                api_error = _redrob_error(wait_payload)
                if api_error:
                    raise ValueError(api_error)
            except (ValueError, TypeError) as error:
                self._fail("RedRob did not complete the request", str(error) + "\n" + stdout)
                return
        self.status.setText("Loading the response…")
        self._api(
            "GET",
            f"/api/session/{self._session_id}/context",
            None,
            self._after_context,
        )

    def _after_context(self, exit_code, stdout, stderr):
        if exit_code != 0:
            self._fail("Could not load the RedRob response", stderr or stdout)
            return
        try:
            context = _decode_json(stdout)
            api_error = _redrob_error(context)
            if api_error:
                raise ValueError(api_error)
            response = _assistant_text(context)
        except (ValueError, TypeError) as error:
            self._fail("Could not parse the RedRob response", str(error) + "\n" + stdout)
            return
        self._last_response = response
        self._append("RedRob", response)
        self.run_code_button.setEnabled(bool(self._extract_python()))
        self._set_busy(False, "Ready — response generated by the RedRob Code engine.")

    def _fail(self, title, detail):
        self._append("OpenCAD", title + ("\n" + detail.strip() if detail.strip() else ""))
        self._set_busy(False, title)

    def _process_running(self):
        return self._process is not None and self._process.state() != QtCore.QProcess.NotRunning

    def stop(self):
        self._operation_id += 1
        if self._process_running():
            self._process.kill()
        if self._session_id:
            process = QtCore.QProcess(self)
            self._interrupt_process = process
            process.setProgram(self._binary())
            process.setArguments(
                [
                    "api",
                    "POST",
                    f"/api/session/{self._session_id}/interrupt",
                    "--header",
                    "x-redrob-directory:"
                    + (
                        self._session_binding[2]
                        if self._session_binding
                        else _working_directory()
                    ),
                ]
            )
            process.start()
        self._set_busy(False, "Request interrupted.")

    def _extract_python(self):
        blocks = re.findall(r"```(?:python|py)\s*\n(.*?)```", self._last_response, re.DOTALL | re.IGNORECASE)
        return blocks[-1].strip() if blocks else ""

    def run_python(self):
        code = self._extract_python()
        if not code:
            return
        choice = QtWidgets.QMessageBox.warning(
            self,
            "Run AI-generated Python?",
            "This code can modify documents and files. Run it only after reviewing the RedRob "
            "response. Continue?",
            QtWidgets.QMessageBox.Yes | QtWidgets.QMessageBox.No,
            QtWidgets.QMessageBox.No,
        )
        if choice != QtWidgets.QMessageBox.Yes:
            return
        document = App.ActiveDocument
        document_name = document.Name if document else None
        transaction_open = False
        if document:
            document.openTransaction("RedRob AI operation")
            transaction_open = True
        try:
            # Deliberately execute only after the explicit confirmation above; this is the
            # established FreeCAD automation boundary, not data deserialization.
            exec(  # noqa: S102
                compile(code, "<RedRob OpenCAD agent>", "exec"), {"App": App, "Gui": Gui}
            )
            current_documents = App.listDocuments()
            if document_name and document_name in current_documents:
                current_document = current_documents[document_name]
                current_document.recompute()
                invalid = [obj for obj in current_document.Objects if not obj.isValid()]
                if invalid:
                    details = "; ".join(
                        f"{obj.Label}: {obj.getStatusString()}" for obj in invalid[:10]
                    )
                    raise RuntimeError("Document recompute left invalid objects: " + details)
            Gui.updateGui()
            if document_name and document_name in App.listDocuments():
                App.getDocument(document_name).commitTransaction()
                transaction_open = False
            self._append("OpenCAD", "The reviewed Python script completed successfully.")
        except Exception as error:  # noqa: BLE001 - generated scripts can raise any exception
            if transaction_open and document_name and document_name in App.listDocuments():
                App.getDocument(document_name).abortTransaction()
            self._append("OpenCAD", "Python execution failed and was not committed: " + str(error))

    def configure(self):
        dialog = QtWidgets.QDialog(self)
        dialog.setWindowTitle("RedRob Code settings")
        form = QtWidgets.QFormLayout(dialog)
        executable = QtWidgets.QLineEdit(self._binary())
        model = QtWidgets.QLineEdit(_parameters().GetString("Model", "redrob/auto"))
        directory = QtWidgets.QLineEdit(_parameters().GetString("WorkingDirectory", ""))
        directory.setPlaceholderText("Active document folder, or home directory")
        form.addRow("Executable", executable)
        form.addRow("Model", model)
        form.addRow("Project directory", directory)
        note = QtWidgets.QLabel(
            "Authentication stays in RedRob Code. Connect once in a terminal with:<br>"
            "<code>redrob providers login --provider redrob</code>"
        )
        note.setWordWrap(True)
        form.addRow(note)
        buttons = QtWidgets.QDialogButtonBox(
            QtWidgets.QDialogButtonBox.Ok | QtWidgets.QDialogButtonBox.Cancel
        )
        buttons.accepted.connect(dialog.accept)
        buttons.rejected.connect(dialog.reject)
        form.addRow(buttons)
        execute_dialog = getattr(dialog, "exec", None) or dialog.exec_
        if execute_dialog() == QtWidgets.QDialog.Accepted:
            _parameters().SetString("Executable", executable.text().strip() or "redrob")
            _parameters().SetString("Model", model.text().strip() or "redrob/auto")
            _parameters().SetString("WorkingDirectory", directory.text().strip())
            self.status.setText("RedRob settings saved.")


def show_panel():
    global _panel
    main_window = Gui.getMainWindow()
    dock = main_window.findChild(QtWidgets.QDockWidget, _DOCK_NAME)
    if dock is None:
        dock = QtWidgets.QDockWidget("RedRob AI Agent", main_window)
        dock.setObjectName(_DOCK_NAME)
        dock.setAllowedAreas(QtCore.Qt.LeftDockWidgetArea | QtCore.Qt.RightDockWidgetArea)
        _panel = RedRobPanel(dock)
        dock.setWidget(_panel)
        main_window.addDockWidget(QtCore.Qt.RightDockWidgetArea, dock)
    else:
        _panel = dock.widget()
    dock.show()
    dock.raise_()
    return _panel


def stop_active_request():
    if _panel is not None:
        _panel.stop()
