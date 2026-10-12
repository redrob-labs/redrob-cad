// redrob_cad_setup -- the branded Windows setup and uninstaller for Redrob CAD.
//
// Ported from redrob-browser's installer/windows/redrob_setup.cpp: the same window, design and
// states, so the two products install the same way. What differs is the payload. The browser wraps
// Chromium's mini_installer; Redrob CAD wraps its own MSI (WiX, packaging/windows/redrob-cad.wxs),
// run silently with msiexec for this Windows account (ALLUSERS=2 MSIINSTALLPERUSER=1: no
// administrator rights, %LOCALAPPDATA%\Programs\Redrob CAD).
//
// Design system (redrob-design-system), binding:
//   * Ground "deep" (20-color.md): Redrob Black #0A0B0C, type gray-1 / ink-muted, a white pill CTA.
//   * "Blue acts": the progress fill is action-primary, Redrob Blue #2B52FF.
//   * Redrob CAD has no product color (the family is complete at seven), so the window carries the
//     Redrob lockup with the name set after it (assets/Products/README.md, "Lockups"): Solid Mono
//     White on the deep ground, nothing tinted.
//   * Foot light: `wash-edge` from the brand gradient, pre-rendered into res/background.png.
//   * Type: Pretendard SemiBold for Latin, Wanted Sans SemiBold when Windows is in Korean
//     (35-korean.md: a Korean face sets Korean). Both embedded (OFL), loaded in memory through
//     DirectWrite; Segoe UI Variable if that fails.
//
// Uninstall: the same binary, built without a payload, ships inside the MSI as
// redrob-cad-uninstall.exe, and the Settings > Apps entry the MSI writes points at it with
// --uninstall. It copies itself to %TEMP% first (msiexec must be free to delete the original),
// asks, then runs msiexec /x for the installed product (found by the MSI's UpgradeCode), and
// optionally deletes the settings folder (%APPDATA%\Redrob CAD: settings.json, account.json).
//
// Redrob Code (opt-in, off by default): exactly as the browser does it. The latest release's
// redrob-code-x64-setup.exe, checked against that release's SHA256SUMS, installed per-user into
// %LOCALAPPDATA%\Redrob\bin, which Redrob CAD's agent lookup reads.
//
// Flags:
//   --with-redrob-code   start with "Also install Redrob Code" ticked (still a choice).
//   --demo               the whole install sequence on a timer, NO payload, no download.
//   --demo-error         the error state immediately.
//   --demo-code-error    --demo, ending on the "Redrob Code was not installed" screen.
//   --uninstall          remove Redrob CAD (what Settings > Apps runs).
//   --demo-uninstall     the uninstall sequence on a timer, removing nothing.
//   --lang en|ko         force the window language (default: the Windows display language).
//   (no flag)            install the embedded MSI for real.

#ifndef UNICODE
#define UNICODE
#endif
#ifndef NOMINMAX
#define NOMINMAX  // keep windows.h's min/max macros from breaking std::min/std::max
#endif

#include <windows.h>
#include <windowsx.h>
#include <dwmapi.h>
#include <d2d1.h>
#include <d2d1helper.h>
#include <dwrite_3.h>
#include <wincodec.h>
#include <shellapi.h>
#include <shlobj.h>
#include <tlhelp32.h>
#include <wininet.h>
#include <urlmon.h>
#include <bcrypt.h>
#include <msi.h>
#include <string>
#include <algorithm>
#include <cctype>
#include <memory>
#include <vector>

#include "resource.h"

#pragma comment(lib, "dwmapi.lib")
#pragma comment(lib, "d2d1.lib")
#pragma comment(lib, "dwrite.lib")
#pragma comment(lib, "windowscodecs.lib")
#pragma comment(lib, "shell32.lib")
#pragma comment(lib, "shlwapi.lib")
#pragma comment(lib, "ole32.lib")
#pragma comment(lib, "user32.lib")
#pragma comment(lib, "gdi32.lib")
#pragma comment(lib, "advapi32.lib")
#pragma comment(lib, "urlmon.lib")
#pragma comment(lib, "wininet.lib")
#pragma comment(lib, "bcrypt.lib")
#pragma comment(lib, "msi.lib")

namespace {

// ---- design-system colors (deep ground) -----------------------------------
constexpr D2D1_COLOR_F kRedrobBlack = {0x0A / 255.0f, 0x0B / 255.0f, 0x0C / 255.0f, 1.0f};
constexpr D2D1_COLOR_F kInkPrimary  = {0xF8 / 255.0f, 0xF9 / 255.0f, 0xFB / 255.0f, 1.0f}; // gray-1
constexpr D2D1_COLOR_F kInkMuted    = {0x83 / 255.0f, 0x8A / 255.0f, 0x97 / 255.0f, 1.0f}; // ink-muted dark
constexpr D2D1_COLOR_F kActionBlue  = {0x2B / 255.0f, 0x52 / 255.0f, 0xFF / 255.0f, 1.0f}; // action-primary
constexpr D2D1_COLOR_F kTrack       = {0xFF / 255.0f, 0xFF / 255.0f, 0xFF / 255.0f, 0.14f}; // border-subtle on dark
constexpr D2D1_COLOR_F kDanger      = {0xFF / 255.0f, 0x4B / 255.0f, 0x4B / 255.0f, 1.0f}; // accent-red-3 / status-danger
constexpr D2D1_COLOR_F kWhite       = {1.0f, 1.0f, 1.0f, 1.0f};

// ---- geometry (logical px at 96 DPI) ---------------------------------------
constexpr int   kBaseW   = 660;
constexpr int   kBaseH   = 400;
constexpr float kRadius  = 14.0f;   // rounded corners
constexpr float kCaption = 44.0f;   // top drag strip height

// window messages / timers
constexpr UINT_PTR kTickTimer = 1;
constexpr UINT     kTickMs     = 16;   // ~60fps for the eased bar / fades
constexpr UINT_PTR kPayloadMsgDone = (WM_APP + 1);
constexpr UINT_PTR kUninstallMsgDone = (WM_APP + 2);
constexpr UINT_PTR kStageMsg = (WM_APP + 3);  // the worker moved on: wParam is the Stage

// Redrob Code's release (public repo). `latest/download/<asset>` redirects to the newest release's asset.
constexpr wchar_t kCodeSetupUrl[] =
    L"https://github.com/redrob-labs/redrob-code/releases/latest/download/redrob-code-x64-setup.exe";
constexpr wchar_t kCodeSumsUrl[] =
    L"https://github.com/redrob-labs/redrob-code/releases/latest/download/SHA256SUMS";
constexpr char kCodeSetupAsset[] = "redrob-code-x64-setup.exe";
// Share of the bar the Redrob Code step owns when chosen; Redrob CAD fills the rest.
constexpr double kCodeShare = 0.35;

// The MSI's UpgradeCode (packaging/windows/redrob-cad.wxs): constant across versions, so it finds
// whichever Redrob CAD is installed.
constexpr wchar_t kUpgradeCode[] = L"{02B4F402-B78B-4D1B-B018-B59C5308CDD8}";
// msiexec results that mean "it is done" (winerror.h).
constexpr DWORD kMsiRebootRequired = 3010;   // ERROR_SUCCESS_REBOOT_REQUIRED
constexpr DWORD kMsiRebootInitiated = 1641;  // ERROR_SUCCESS_REBOOT_INITIATED
constexpr DWORD kMsiUnknownProduct = 1605;   // ERROR_UNKNOWN_PRODUCT: nothing left to remove
// Our own result: Redrob CAD is open, so files are in use. Shown as a request, not a failure code.
constexpr DWORD kAppRunning = 0xA0000001;

// The window's language. Korean when Windows is, unless --lang says otherwise.
bool g_ko = false;
const wchar_t* T(const wchar_t* en, const wchar_t* ko) { return g_ko ? ko : en; }

enum class Stage { Preparing, Installing, Opening, Error, Confirm, Removing, Removed,
                   Welcome, InstallingCode, CodeFailed };
enum class Mode { Install, Uninstall };

// A tiny COM pointer so cleanup is not a maze of gotos. Move-only: a plain
// memberwise copy would double-Release (and a function returning one by value
// would hand back an already-freed interface).
template <class T> struct ComPtr {
  T* p = nullptr;
  ComPtr() = default;
  ComPtr(const ComPtr&) = delete;
  ComPtr& operator=(const ComPtr&) = delete;
  ComPtr(ComPtr&& o) noexcept : p(o.p) { o.p = nullptr; }
  ComPtr& operator=(ComPtr&& o) noexcept {
    if (std::addressof(o) != this) { if (p) p->Release(); p = o.p; o.p = nullptr; }
    return *this;
  }
  ~ComPtr() { if (p) p->Release(); }
  T** operator&() { return &p; }
  T* operator->() const { return p; }
  explicit operator bool() const { return p != nullptr; }
  void reset() { if (p) { p->Release(); p = nullptr; } }
};

std::wstring g_statusOverride;

// ---------------------------------------------------------------------------
// The app object. One window, Direct2D render target, DirectWrite text, WIC for
// the PNG resources, and a worker thread that runs the payload.
// ---------------------------------------------------------------------------
class SetupApp {
 public:
  SetupApp(bool demo, bool demoError, Mode mode, bool withCode, bool demoCodeError)
      : demo_(demo), demoError_(demoError), mode_(mode), installCode_(withCode), demoCodeError_(demoCodeError) {}

  int Run(HINSTANCE hinst);

 private:
  static LRESULT CALLBACK WndProcThunk(HWND, UINT, WPARAM, LPARAM);
  LRESULT WndProc(UINT, WPARAM, LPARAM);

  void CreateDeviceResources();
  void DiscardDeviceResources();
  void OnPaint();
  void OnTick();
  void LayoutText();
  void LoadFonts();
  RECT ToDevice(float x, float y, float w, float h);
  void computeButtonRects(float minX, float closeX, float cy);

  ComPtr<ID2D1Bitmap> LoadPngResource(int resId, ComPtr<ID2D1HwndRenderTarget>& rt);

  // payload
  static DWORD WINAPI PayloadThunk(LPVOID self);
  DWORD RunPayload();
  void LaunchApp();
  bool ExtractPayload(std::wstring& outPath);

  // install (Redrob Code, opt-in)
  void StartInstall();
  std::wstring InstallRedrobCode();  // empty on success, else why not (shown to the user)

  // uninstall
  static DWORD WINAPI UninstallThunk(LPVOID self);
  DWORD RunUninstall();
  void StartUninstall();
  void ScheduleSelfCleanup();

  void SetStage(Stage s);
  void RequestClose(bool force);

  HINSTANCE hinst_ = nullptr;
  HWND hwnd_ = nullptr;
  UINT dpi_ = 96;
  bool demo_ = false;
  bool demoError_ = false;
  Mode mode_ = Mode::Install;
  bool deleteData_ = false;   // the confirm screen's "also delete settings and workspace key"
  bool installCode_ = false;  // the welcome screen's "also install Redrob Code" -- opt-in, off by default
  bool demoCodeError_ = false;
  bool codePresent_ = false;  // Redrob Code is already installed: the welcome screen says so, offers nothing
  bool codeDone_ = false;     // installed by this run; a Retry does not install it twice
  std::wstring codeError_;    // set by the worker before it posts kPayloadMsgDone

  // Direct2D / DirectWrite / WIC
  ComPtr<ID2D1Factory>        d2dFactory_;
  ComPtr<IDWriteFactory>      dwFactory_;
  ComPtr<IWICImagingFactory>  wicFactory_;
  ComPtr<ID2D1HwndRenderTarget> rt_;
  ComPtr<ID2D1Bitmap>         bgBitmap_;
  ComPtr<ID2D1Bitmap>         lockupBitmap_;
  ComPtr<IDWriteTextFormat>   fmtStatus_;
  ComPtr<IDWriteTextFormat>   fmtButton_;
  ComPtr<IDWriteFontCollection1> fontColl_;
  std::wstring                fontFamily_ = L"Segoe UI Variable Display";

  // state
  Stage  stage_ = Stage::Preparing;
  double progress_ = 0.0;      // 0..1 displayed
  double progressTarget_ = 0.0;
  ULONGLONG startTick_ = 0;
  ULONGLONG openedAt_ = 0;
  ULONGLONG stageTick_ = 0;    // when the current stage began; each install step ramps from here
  bool payloadDone_ = false;
  DWORD payloadExit_ = 0;
  HANDLE payloadThread_ = nullptr;
  std::wstring errorMessage_;
  std::wstring extractedPayload_;
  bool closing_ = false;

  // hit-tested button rects (device px), recomputed each paint
  RECT rcMin_{}, rcClose_{}, rcRetry_{}, rcErrClose_{}, rcCheck_{};
  bool confirmingQuit_ = false;
};

SetupApp* g_app = nullptr;

std::wstring StatusText(Stage s) {
  if (!g_statusOverride.empty()) return g_statusOverride;
  switch (s) {
    case Stage::Preparing:  return T(L"Preparing\u2026", L"준비하는 중\u2026");
    case Stage::Installing: return T(L"Installing\u2026", L"설치하는 중\u2026");
    case Stage::Opening:    return T(L"Opening Redrob CAD\u2026", L"Redrob CAD를 여는 중\u2026");
    case Stage::Error:      return T(L"Something went wrong", L"문제가 생겼어요");
    case Stage::Confirm:    return T(L"Uninstall Redrob CAD?", L"Redrob CAD를 제거할까요?");
    case Stage::Removing:   return T(L"Removing Redrob CAD\u2026", L"Redrob CAD를 제거하는 중\u2026");
    case Stage::Removed:    return T(L"Redrob CAD has been removed", L"Redrob CAD를 제거했어요");
    case Stage::Welcome:    return T(L"Install Redrob CAD", L"Redrob CAD 설치");
    case Stage::InstallingCode: return T(L"Installing Redrob Code\u2026", L"Redrob Code를 설치하는 중\u2026");
    case Stage::CodeFailed: return T(L"Redrob CAD is installed", L"Redrob CAD를 설치했어요");
  }
  return L"";
}

// ---------------------------------------------------------------------------

// Whether Redrob Code is already on this PC: its installer's folder (%LOCALAPPDATA%\Redrob\bin), the
// install.sh location (%USERPROFILE%\.redrob\bin), or anywhere on PATH. The same places Redrob CAD's
// agent lookup tries (apps/cadcraft/src/acp.rs), so "already installed" here means the agent finds it.
static bool RedrobCodePresent() {
  auto exists = [](const std::wstring& p) { return GetFileAttributesW(p.c_str()) != INVALID_FILE_ATTRIBUTES; };
  wchar_t dir[MAX_PATH];
  if (SUCCEEDED(SHGetFolderPathW(nullptr, CSIDL_LOCAL_APPDATA, nullptr, 0, dir)) &&
      exists(std::wstring(dir) + L"\\Redrob\\bin\\redrob.exe"))
    return true;
  if (SUCCEEDED(SHGetFolderPathW(nullptr, CSIDL_PROFILE, nullptr, 0, dir)) &&
      exists(std::wstring(dir) + L"\\.redrob\\bin\\redrob.exe"))
    return true;
  wchar_t found[MAX_PATH];
  return SearchPathW(nullptr, L"redrob.exe", nullptr, MAX_PATH, found, nullptr) > 0;
}

int SetupApp::Run(HINSTANCE hinst) {
  hinst_ = hinst;
  g_app = this;

  SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
  HRESULT hr = CoInitializeEx(nullptr, COINIT_APARTMENTTHREADED);
  (void)hr;

  D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, &d2dFactory_);
  DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED, __uuidof(IDWriteFactory),
                      reinterpret_cast<IUnknown**>(&dwFactory_));
  CoCreateInstance(CLSID_WICImagingFactory, nullptr, CLSCTX_INPROC_SERVER,
                   IID_PPV_ARGS(&wicFactory_));

  WNDCLASSEX wc = {sizeof(wc)};
  wc.style = CS_HREDRAW | CS_VREDRAW;
  wc.lpfnWndProc = &SetupApp::WndProcThunk;
  wc.hInstance = hinst;
  wc.hCursor = LoadCursor(nullptr, IDC_ARROW);
  wc.lpszClassName = L"RedrobCadSetupWindow";
  wc.hIcon = LoadIconW(hinst, MAKEINTRESOURCEW(IDI_APPICON));
  wc.hIconSm = wc.hIcon;
  RegisterClassEx(&wc);

  // Center on the primary monitor's work area.
  dpi_ = GetDpiForSystem();
  int w = MulDiv(kBaseW, dpi_, 96);
  int h = MulDiv(kBaseH, dpi_, 96);
  RECT wa{};
  SystemParametersInfo(SPI_GETWORKAREA, 0, &wa, 0);
  int x = wa.left + ((wa.right - wa.left) - w) / 2;
  int y = wa.top + ((wa.bottom - wa.top) - h) / 2;

  // Borderless (WS_POPUP). WS_THICKFRAME is kept out; a DWM shadow is attached
  // separately so a popup still casts one.
  hwnd_ = CreateWindowExW(WS_EX_APPWINDOW, wc.lpszClassName,
                          mode_ == Mode::Uninstall ? T(L"Uninstall Redrob CAD", L"Redrob CAD 제거") : T(L"Redrob CAD Setup", L"Redrob CAD 설치"),
                          WS_POPUP, x, y, w, h, nullptr, nullptr, hinst, this);
  if (!hwnd_) return 2;

  // DWM: a real shadow on a borderless window + rounded corners (Win11).
  MARGINS m = {1, 1, 1, 1};
  DwmExtendFrameIntoClientArea(hwnd_, &m);
  DWM_WINDOW_CORNER_PREFERENCE corner = DWMWCP_ROUND;
  DwmSetWindowAttribute(hwnd_, DWMWA_WINDOW_CORNER_PREFERENCE, &corner, sizeof(corner));
  BOOL dark = TRUE;
  DwmSetWindowAttribute(hwnd_, DWMWA_USE_IMMERSIVE_DARK_MODE, &dark, sizeof(dark));

  LayoutText();

  startTick_ = GetTickCount64();
  if (mode_ == Mode::Uninstall) {
    SetStage(Stage::Confirm);
  } else if (demoError_) {
    errorMessage_ = T(L"The installer could not be started. Please try again.", L"설치를 시작하지 못했어요. 다시 시도해 주세요.");
    SetStage(Stage::Error);
  } else {
    // Nothing is installed until the person presses Install: the welcome screen is where Redrob Code is
    // offered, and an opt-in that has already started is not an opt-in.
    if (!demo_) codePresent_ = RedrobCodePresent();
    if (codePresent_) installCode_ = false;
    SetStage(Stage::Welcome);
  }

  ShowWindow(hwnd_, SW_SHOW);
  UpdateWindow(hwnd_);

  MSG msg;
  while (GetMessage(&msg, nullptr, 0, 0) > 0) {
    TranslateMessage(&msg);
    DispatchMessage(&msg);
  }
  if (payloadThread_) {
    WaitForSingleObject(payloadThread_, 5000);
    CloseHandle(payloadThread_);
  }
  // Every COM object goes BEFORE CoUninitialize. Left to the destructor, the WIC factory was released after
  // COM was torn down, and the process died with an access violation on its way out of a successful install.
  DiscardDeviceResources();
  fmtStatus_.reset();
  fmtButton_.reset();
  fontColl_.reset();
  wicFactory_.reset();
  dwFactory_.reset();
  d2dFactory_.reset();
  CoUninitialize();
  return (int)msg.wParam;
}

// The embedded faces (IDR_FONT_SANS, IDR_FONT_KR) as one in-memory DirectWrite collection (Windows 10
// 1703+). GDI's AddFontMemResourceEx would not do: DirectWrite never sees GDI private fonts.
void SetupApp::LoadFonts() {
  ComPtr<IDWriteFactory5> f5;
  if (FAILED(dwFactory_->QueryInterface(__uuidof(IDWriteFactory5), reinterpret_cast<void**>(&f5)))) return;
  ComPtr<IDWriteInMemoryFontFileLoader> loader;
  if (FAILED(f5->CreateInMemoryFontFileLoader(&loader))) return;
  if (FAILED(f5->RegisterFontFileLoader(loader.p))) return;
  ComPtr<IDWriteFontSetBuilder1> builder;
  if (FAILED(f5->CreateFontSetBuilder(&builder))) return;
  int added = 0;
  for (int id : {IDR_FONT_SANS, IDR_FONT_KR}) {
    HRSRC res = FindResourceW(hinst_, MAKEINTRESOURCEW(id), RT_RCDATA);
    if (!res) continue;
    void* data = LockResource(LoadResource(hinst_, res));
    DWORD sz = SizeofResource(hinst_, res);
    if (!data || !sz) continue;
    ComPtr<IDWriteFontFile> file;
    if (SUCCEEDED(loader->CreateInMemoryFontFileReference(f5.p, data, sz, nullptr, &file)) &&
        SUCCEEDED(builder->AddFontFile(file.p)))
      ++added;
  }
  if (!added) return;
  ComPtr<IDWriteFontSet> set;
  if (FAILED(builder->CreateFontSet(&set))) return;
  ComPtr<IDWriteFontCollection1> coll;
  if (FAILED(f5->CreateFontCollectionFromFontSet(set.p, &coll))) return;
  // The family name as DirectWrite's weight-width-slope model reports it: try the typographic name,
  // then the legacy one, and keep whichever the collection holds.
  const wchar_t* wanted[] = {g_ko ? L"Wanted Sans" : L"Pretendard", g_ko ? L"Wanted Sans SemiBold" : L"Pretendard SemiBold"};
  for (const wchar_t* name : wanted) {
    UINT32 index = 0;
    BOOL exists = FALSE;
    if (SUCCEEDED(coll->FindFamilyName(name, &index, &exists)) && exists) {
      fontFamily_ = name;
      fontColl_.reset();
      fontColl_.p = coll.p;
      coll.p = nullptr;
      return;
    }
  }
}

void SetupApp::LayoutText() {
  LoadFonts();
  const wchar_t* locale = g_ko ? L"ko-kr" : L"en-us";
  dwFactory_->CreateTextFormat(fontFamily_.c_str(), fontColl_.p, DWRITE_FONT_WEIGHT_SEMI_BOLD,
                               DWRITE_FONT_STYLE_NORMAL, DWRITE_FONT_STRETCH_NORMAL,
                               17.0f, locale, &fmtStatus_);
  if (fmtStatus_) {
    fmtStatus_->SetTextAlignment(DWRITE_TEXT_ALIGNMENT_CENTER);
    fmtStatus_->SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER);
  }
  dwFactory_->CreateTextFormat(fontFamily_.c_str(), fontColl_.p, DWRITE_FONT_WEIGHT_SEMI_BOLD,
                               DWRITE_FONT_STYLE_NORMAL, DWRITE_FONT_STRETCH_NORMAL,
                               13.0f, locale, &fmtButton_);
  if (fmtButton_) {
    fmtButton_->SetTextAlignment(DWRITE_TEXT_ALIGNMENT_CENTER);
    fmtButton_->SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER);
    fmtButton_->SetWordWrapping(DWRITE_WORD_WRAPPING_WRAP);
  }
}

LRESULT CALLBACK SetupApp::WndProcThunk(HWND hwnd, UINT msg, WPARAM wp, LPARAM lp) {
  if (msg == WM_NCCREATE) {
    auto* cs = reinterpret_cast<CREATESTRUCT*>(lp);
    SetWindowLongPtr(hwnd, GWLP_USERDATA, (LONG_PTR)cs->lpCreateParams);
  }
  auto* self = reinterpret_cast<SetupApp*>(GetWindowLongPtr(hwnd, GWLP_USERDATA));
  if (self) { self->hwnd_ = hwnd; return self->WndProc(msg, wp, lp); }
  return DefWindowProc(hwnd, msg, wp, lp);
}

LRESULT SetupApp::WndProc(UINT msg, WPARAM wp, LPARAM lp) {
  switch (msg) {
    case WM_PAINT: OnPaint(); return 0;
    case WM_ERASEBKGND: return 1;

    case WM_TIMER:
      if (wp == kTickTimer) OnTick();
      return 0;

    case kStageMsg:
      if (stage_ != Stage::Error) SetStage((Stage)wp);
      return 0;

    case kPayloadMsgDone: {
      payloadDone_ = true;
      payloadExit_ = (DWORD)wp;
      const bool ok = payloadExit_ == 0 || payloadExit_ == kMsiRebootRequired || payloadExit_ == kMsiRebootInitiated;
      if (ok) payloadExit_ = 0;
      if (payloadExit_ == 0 && !codeError_.empty()) {
        // Redrob CAD is installed and open; only Redrob Code is missing. Say so and wait for Close,
        // rather than closing on a timer over a message the person has to read.
        progressTarget_ = progress_ = 1.0;
        SetStage(Stage::CodeFailed);
      } else if (payloadExit_ == 0) {
        progressTarget_ = 1.0;
        SetStage(Stage::Opening);
      } else if (payloadExit_ == kAppRunning) {
        errorMessage_ = T(L"Redrob CAD is open. Save your drawings, close it, then press Retry.",
                          L"Redrob CAD가 열려 있어요. 도면을 저장하고 닫은 뒤 다시 시도를 눌러 주세요.");
        SetStage(Stage::Error);
      } else {
        errorMessage_ = (g_ko ? L"설치를 마치지 못했어요(코드 " : L"Setup did not complete (code ") +
                        std::to_wstring(payloadExit_) +
                        T(L"). Please try again.", L"). 다시 시도해 주세요.");
        SetStage(Stage::Error);
      }
      return 0;
    }

    case kUninstallMsgDone: {
      const DWORD code = (DWORD)wp;
      if (code == 0 || code == kMsiRebootRequired || code == kMsiRebootInitiated || code == kMsiUnknownProduct) {
        progressTarget_ = 1.0;
        progress_ = 1.0;
        SetStage(Stage::Removed);
        if (!demo_) ScheduleSelfCleanup();
      } else if (code == kAppRunning) {
        errorMessage_ = T(L"Redrob CAD is open. Save your drawings, close it, then press Retry.",
                          L"Redrob CAD가 열려 있어요. 도면을 저장하고 닫은 뒤 다시 시도를 눌러 주세요.");
        SetStage(Stage::Error);
      } else {
        errorMessage_ = (g_ko ? L"Redrob CAD를 제거하지 못했어요(코드 " : L"Redrob CAD could not be removed (code ") +
                        std::to_wstring(code) + T(L"). Please try again.", L"). 다시 시도해 주세요.");
        SetStage(Stage::Error);
      }
      return 0;
    }

    case WM_GETDPISCALEDSIZE: return TRUE;
    case WM_DPICHANGED: {
      dpi_ = HIWORD(wp);
      RECT* r = reinterpret_cast<RECT*>(lp);
      SetWindowPos(hwnd_, nullptr, r->left, r->top, r->right - r->left, r->bottom - r->top,
                   SWP_NOZORDER | SWP_NOACTIVATE);
      DiscardDeviceResources();
      InvalidateRect(hwnd_, nullptr, FALSE);
      return 0;
    }

    case WM_SIZE: DiscardDeviceResources(); return 0;

    case WM_LBUTTONDOWN: {
      POINT pt{GET_X_LPARAM(lp), GET_Y_LPARAM(lp)};
      if (PtInRect(&rcClose_, pt)) { RequestClose(false); return 0; }
      if (PtInRect(&rcMin_, pt)) { ShowWindow(hwnd_, SW_MINIMIZE); return 0; }
      if (stage_ == Stage::Confirm) {
        if (PtInRect(&rcCheck_, pt)) { deleteData_ = !deleteData_; InvalidateRect(hwnd_, nullptr, FALSE); return 0; }
        if (PtInRect(&rcRetry_, pt)) { StartUninstall(); return 0; }       // "Uninstall"
        if (PtInRect(&rcErrClose_, pt)) { RequestClose(true); return 0; }  // "Cancel"
      }
      if (stage_ == Stage::Welcome) {
        if (!codePresent_ && PtInRect(&rcCheck_, pt)) {
          installCode_ = !installCode_;
          InvalidateRect(hwnd_, nullptr, FALSE);
          return 0;
        }
        if (PtInRect(&rcRetry_, pt)) { StartInstall(); return 0; }         // "Install"
        if (PtInRect(&rcErrClose_, pt)) { RequestClose(true); return 0; }  // "Cancel"
      }
      if (stage_ == Stage::CodeFailed && PtInRect(&rcRetry_, pt)) { RequestClose(true); return 0; }  // "Close"
      if (stage_ == Stage::Removed && PtInRect(&rcRetry_, pt)) { RequestClose(true); return 0; }  // "Close"
      if (stage_ == Stage::Error && mode_ == Mode::Uninstall && PtInRect(&rcRetry_, pt)) {
        errorMessage_.clear();
        SetStage(Stage::Confirm);  // back to the question, so a retry is still a choice
        return 0;
      }
      if (stage_ == Stage::Error && PtInRect(&rcRetry_, pt)) {
        // Retry: restart the sequence. A Redrob Code install that already succeeded is not repeated.
        errorMessage_.clear();
        StartInstall();
        return 0;
      }
      if (stage_ == Stage::Error && PtInRect(&rcErrClose_, pt)) { RequestClose(true); return 0; }
      // Anywhere else on the caption strip drags the window.
      if (pt.y < MulDiv((int)kCaption, dpi_, 96)) {
        ReleaseCapture();
        SendMessage(hwnd_, WM_NCLBUTTONDOWN, HTCAPTION, 0);
      }
      return 0;
    }

    case WM_KEYDOWN:
      if (wp == VK_ESCAPE) { RequestClose(false); return 0; }
      if (stage_ == Stage::Confirm && wp == VK_SPACE) {
        deleteData_ = !deleteData_;
        InvalidateRect(hwnd_, nullptr, FALSE);
        return 0;
      }
      if (stage_ == Stage::Confirm && wp == VK_RETURN) { StartUninstall(); return 0; }
      if (stage_ == Stage::Welcome && wp == VK_SPACE && !codePresent_) {
        installCode_ = !installCode_;
        InvalidateRect(hwnd_, nullptr, FALSE);
        return 0;
      }
      if (stage_ == Stage::Welcome && wp == VK_RETURN) { StartInstall(); return 0; }
      if (stage_ == Stage::CodeFailed && wp == VK_RETURN) { RequestClose(true); return 0; }
      return 0;

    case WM_SYSKEYDOWN:
      if (wp == VK_F4) { RequestClose(false); return 0; }  // Alt+F4
      return DefWindowProc(hwnd_, msg, wp, lp);

    case WM_CLOSE:
      RequestClose(false);
      return 0;

    case WM_DESTROY:
      KillTimer(hwnd_, kTickTimer);
      DiscardDeviceResources();
      PostQuitMessage(0);
      return 0;
  }
  return DefWindowProc(hwnd_, msg, wp, lp);
}

void SetupApp::RequestClose(bool force) {
  // While installing, Esc / close ask for confirmation first (do not abandon an
  // install silently). In demo and error states, close immediately.
  bool installing = (stage_ == Stage::Installing || stage_ == Stage::Preparing ||
                     stage_ == Stage::InstallingCode) && !demo_ && !force && mode_ == Mode::Install;
  // A removal cannot be safely abandoned halfway; closing waits for it to finish.
  if (stage_ == Stage::Removing && !force) return;
  if (installing) {
    int r = MessageBoxW(hwnd_,
        T(L"Redrob CAD is still installing. Stop the installation and quit?",
          L"아직 Redrob CAD를 설치하고 있어요. 설치를 멈추고 닫을까요?"),
        T(L"Redrob CAD Setup", L"Redrob CAD 설치"), MB_YESNO | MB_ICONWARNING | MB_DEFBUTTON2);
    if (r != IDYES) return;
  }
  DestroyWindow(hwnd_);
}

void SetupApp::SetStage(Stage s) {
  if (s != stage_) stageTick_ = GetTickCount64();
  stage_ = s;
  g_statusOverride.clear();
  if (s == Stage::Opening) openedAt_ = GetTickCount64();
  if (s == Stage::Error || s == Stage::Confirm || s == Stage::Removed || s == Stage::Welcome ||
      s == Stage::CodeFailed)
    KillTimer(hwnd_, kTickTimer);
  if (hwnd_) InvalidateRect(hwnd_, nullptr, FALSE);
}

// The Install button (and Retry). The worker installs Redrob Code first when it was chosen, then Redrob CAD.
void SetupApp::StartInstall() {
  progress_ = progressTarget_ = 0.0;
  startTick_ = GetTickCount64();
  payloadDone_ = false;
  codeError_.clear();
  SetStage(Stage::Preparing);
  SetTimer(hwnd_, kTickTimer, kTickMs, nullptr);
  if (!demo_) {
    if (payloadThread_) { WaitForSingleObject(payloadThread_, 5000); CloseHandle(payloadThread_); }
    payloadThread_ = CreateThread(nullptr, 0, &SetupApp::PayloadThunk, this, 0, nullptr);
  }
}

static double EaseOut(double t) { return 1 - (1 - t) * (1 - t); }

void SetupApp::OnTick() {
  ULONGLONG elapsed = GetTickCount64() - startTick_;

  if (mode_ == Mode::Uninstall) {
    // Removal reports no progress either: the same eased ramp, over ~12s, capped until msiexec exits.
    if (stage_ == Stage::Removing) {
      double t = std::min(1.0, (GetTickCount64() - startTick_) / 12000.0);
      progressTarget_ = 0.90 * (1 - (1 - t) * (1 - t));
    }
  } else if (demo_ && stage_ != Stage::Error) {
    // Scripted demo, same shape as a real run. With Redrob Code chosen:
    //   Preparing (0-0.8s) -> Installing Redrob Code (0.8-4s, bar to ~kCodeShare)
    //   -> Installing (4-8s, bar to ~0.9) -> Opening (8-9.5s) -> close, or the "Redrob Code was not
    //   installed" screen with --demo-code-error. Without it, the Redrob Code step is skipped (as before).
    const ULONGLONG code = installCode_ ? 3200 : 0;
    if (elapsed < 800) { if (stage_ != Stage::Preparing) SetStage(Stage::Preparing); progressTarget_ = 0.04; }
    else if (elapsed < 800 + code) { if (stage_ != Stage::InstallingCode) SetStage(Stage::InstallingCode);
      progressTarget_ = kCodeShare * 0.95 * EaseOut((elapsed - 800) / (double)code); }
    else if (elapsed < 5000 + code) { if (stage_ != Stage::Installing) SetStage(Stage::Installing);
      const double base = installCode_ ? kCodeShare : 0.0;
      progressTarget_ = base + (0.90 - base) * EaseOut((elapsed - 800 - code) / 4200.0); }
    else if (demoCodeError_ && installCode_) {
      codeError_ = L"its download did not match the release checksum";
      progressTarget_ = progress_ = 1.0;
      SetStage(Stage::CodeFailed);
      return;
    }
    else if (elapsed < 6500 + code) { if (stage_ != Stage::Opening) SetStage(Stage::Opening); progressTarget_ = 1.0; }
    else { DestroyWindow(hwnd_); return; }
  } else if (!demo_) {
    // Real run. The worker posts each step as it starts it (kStageMsg); each step's bar is a time-based
    // eased ramp, capped until the step's process exits. Without Redrob Code, Preparing turns into
    // Installing after 0.6s as it always did.
    if (stage_ == Stage::Preparing && !installCode_ && elapsed > 600) SetStage(Stage::Installing);
    const double since = (double)(GetTickCount64() - stageTick_);
    if (stage_ == Stage::InstallingCode) {
      progressTarget_ = kCodeShare * 0.95 * EaseOut(std::min(1.0, since / 25000.0));  // ~35 MB download
    }
    if (stage_ == Stage::Installing && !payloadDone_) {
      const double base = (installCode_ && !codePresent_) ? kCodeShare : 0.0;
      progressTarget_ = base + (0.90 - base) * EaseOut(std::min(1.0, since / 30000.0));  // approach over ~30s
    }
    if (stage_ == Stage::Opening) {
      progressTarget_ = 1.0;
      // Close on a fixed delay after Opening began, regardless of the eased bar:
      // Redrob CAD has been launched (LaunchApp),
      // so there is nothing left for this window to do but get out of the way. A fixed
      // wall-clock close guarantees the window never lingers on the user's screen.
      if (GetTickCount64() - openedAt_ > 1400) { DestroyWindow(hwnd_); return; }
    }
  }

  // Ease displayed progress toward the target (motion < 1/3 s per step).
  progress_ += (progressTarget_ - progress_) * 0.12;
  if (progress_ > 0.9999) progress_ = 1.0;
  InvalidateRect(hwnd_, nullptr, FALSE);
}

// ---- rendering -------------------------------------------------------------

void SetupApp::CreateDeviceResources() {
  if (rt_) return;
  RECT rc; GetClientRect(hwnd_, &rc);
  D2D1_SIZE_U size = D2D1::SizeU(rc.right - rc.left, rc.bottom - rc.top);
  D2D1_RENDER_TARGET_PROPERTIES props = D2D1::RenderTargetProperties(
      D2D1_RENDER_TARGET_TYPE_DEFAULT,
      D2D1::PixelFormat(DXGI_FORMAT_B8G8R8A8_UNORM, D2D1_ALPHA_MODE_PREMULTIPLIED));
  HRESULT hr = d2dFactory_->CreateHwndRenderTarget(
      props, D2D1::HwndRenderTargetProperties(hwnd_, size, D2D1_PRESENT_OPTIONS_NONE), &rt_);
  (void)hr;
  if (!rt_) return;
  rt_->SetDpi((float)dpi_, (float)dpi_);
  bgBitmap_ = LoadPngResource(IDR_BACKGROUND_PNG, rt_);
  lockupBitmap_ = LoadPngResource(IDR_LOCKUP_PNG, rt_);
}

void SetupApp::DiscardDeviceResources() {
  bgBitmap_.reset();
  lockupBitmap_.reset();
  rt_.reset();
}

ComPtr<ID2D1Bitmap> SetupApp::LoadPngResource(int resId, ComPtr<ID2D1HwndRenderTarget>& rt) {
  ComPtr<ID2D1Bitmap> out;
  HRSRC res = FindResourceW(hinst_, MAKEINTRESOURCEW(resId), RT_RCDATA);
  if (!res) return out;
  HGLOBAL h = LoadResource(hinst_, res);
  void* data = LockResource(h);
  DWORD sz = SizeofResource(hinst_, res);
  if (!data || !sz) return out;

  ComPtr<IWICStream> stream;
  wicFactory_->CreateStream(&stream);
  stream->InitializeFromMemory((BYTE*)data, sz);
  ComPtr<IWICBitmapDecoder> dec;
  if (FAILED(wicFactory_->CreateDecoderFromStream(stream.p, nullptr,
          WICDecodeMetadataCacheOnLoad, &dec))) return out;
  ComPtr<IWICBitmapFrameDecode> frame;
  dec->GetFrame(0, &frame);
  ComPtr<IWICFormatConverter> conv;
  wicFactory_->CreateFormatConverter(&conv);
  conv->Initialize(frame.p, GUID_WICPixelFormat32bppPBGRA, WICBitmapDitherTypeNone,
                   nullptr, 0.0, WICBitmapPaletteTypeMedianCut);
  rt->CreateBitmapFromWicBitmap(conv.p, nullptr, &out);
  return out;
}

void SetupApp::OnPaint() {
  CreateDeviceResources();
  PAINTSTRUCT ps; BeginPaint(hwnd_, &ps);
  if (!rt_) { EndPaint(hwnd_, &ps); return; }

  rt_->BeginDraw();
  rt_->Clear(kRedrobBlack);

  const float W = (float)kBaseW, H = (float)kBaseH;

  // Background art (deep ground + wash-edge), drawn to fill the window.
  if (bgBitmap_) {
    rt_->DrawBitmap(bgBitmap_.p, D2D1::RectF(0, 0, W, H), 1.0f,
                    D2D1_BITMAP_INTERPOLATION_MODE_LINEAR);
  }

  ComPtr<ID2D1SolidColorBrush> ink, muted, blue, track, danger, white;
  rt_->CreateSolidColorBrush(kInkPrimary, &ink);
  rt_->CreateSolidColorBrush(kInkMuted, &muted);
  rt_->CreateSolidColorBrush(kActionBlue, &blue);
  rt_->CreateSolidColorBrush(kTrack, &track);
  rt_->CreateSolidColorBrush(kDanger, &danger);
  rt_->CreateSolidColorBrush(kWhite, &white);

  // ---- caption glyphs (min / close), top-right -----------------------------
  const float gl = 14.0f;              // glyph half-size
  const float cy = kCaption / 2.0f;
  const float closeX = W - 28.0f;
  const float minX   = W - 64.0f;
  computeButtonRects(minX, closeX, cy);
  // close (x)
  rt_->DrawLine(D2D1::Point2F(closeX - gl / 2, cy - gl / 2),
                D2D1::Point2F(closeX + gl / 2, cy + gl / 2), ink.p, 1.6f);
  rt_->DrawLine(D2D1::Point2F(closeX + gl / 2, cy - gl / 2),
                D2D1::Point2F(closeX - gl / 2, cy + gl / 2), ink.p, 1.6f);
  // minimize (-)
  rt_->DrawLine(D2D1::Point2F(minX - gl / 2, cy),
                D2D1::Point2F(minX + gl / 2, cy), ink.p, 1.6f);

  // ---- lockup (product mark + name), centered upper third ------------------
  if (lockupBitmap_) {
    D2D1_SIZE_F ls = lockupBitmap_->GetSize();
    float lh = 34.0f;                       // >= 18px min screen (10-logo.md)
    float lw = ls.width * (lh / ls.height);
    float lx = (W - lw) / 2.0f;
    float ly = H * 0.30f;
    rt_->DrawBitmap(lockupBitmap_.p, D2D1::RectF(lx, ly, lx + lw, ly + lh), 1.0f,
                    D2D1_BITMAP_INTERPOLATION_MODE_LINEAR);
  }

  // ---- status line, centered ------------------------------------------------
  std::wstring status = StatusText(stage_);
  D2D1_RECT_F statusRc = D2D1::RectF(40, H * 0.46f, W - 40, H * 0.46f + 30);
  if (fmtStatus_) rt_->DrawTextW(status.c_str(), (UINT32)status.size(), fmtStatus_.p, statusRc,
                                 (stage_ == Stage::Error ? danger.p : ink.p));

  ComPtr<ID2D1SolidColorBrush> black;
  rt_->CreateSolidColorBrush(kRedrobBlack, &black);
  // A white pill (the one prominent action, black type) and an outline pill, as the error state draws them.
  auto pill = [&](float x, float y, float w, float h, const wchar_t* label, bool primary) {
    D2D1_ROUNDED_RECT rr = {D2D1::RectF(x, y, x + w, y + h), h / 2, h / 2};
    if (primary) rt_->FillRoundedRectangle(rr, white.p);
    else rt_->DrawRoundedRectangle(rr, track.p, 1.4f);
    if (fmtButton_) rt_->DrawTextW(label, (UINT32)wcslen(label), fmtButton_.p,
                                   D2D1::RectF(x, y, x + w, y + h), primary ? black.p : ink.p);
  };
  const float pw = 120.0f, ph = 38.0f, py = H * 0.72f;

  // A 16px box and its label, centred together as one row. The whole row is the hit target, not just the box.
  auto checkRow = [&](const wchar_t* label, bool checked) {
    float labelW = 200.0f;
    ComPtr<IDWriteTextLayout> layout;
    if (fmtButton_ && SUCCEEDED(dwFactory_->CreateTextLayout(label, (UINT32)wcslen(label), fmtButton_.p,
                                                             W, 24.0f, &layout))) {
      layout->SetTextAlignment(DWRITE_TEXT_ALIGNMENT_LEADING);
      DWRITE_TEXT_METRICS tm{};
      if (SUCCEEDED(layout->GetMetrics(&tm))) labelW = tm.widthIncludingTrailingWhitespace;
    }
    const float box = 16.0f, gap = 10.0f, rowY = H * 0.615f;
    const float rowX = (W - (box + gap + labelW)) / 2.0f;
    D2D1_ROUNDED_RECT boxRR = {D2D1::RectF(rowX, rowY, rowX + box, rowY + box), 4.0f, 4.0f};
    if (checked) {
      rt_->FillRoundedRectangle(boxRR, blue.p);
      rt_->DrawLine(D2D1::Point2F(rowX + 4.0f, rowY + 8.5f), D2D1::Point2F(rowX + 7.0f, rowY + 11.5f), white.p, 1.8f);
      rt_->DrawLine(D2D1::Point2F(rowX + 7.0f, rowY + 11.5f), D2D1::Point2F(rowX + 12.5f, rowY + 5.0f), white.p, 1.8f);
    } else {
      rt_->DrawRoundedRectangle(boxRR, muted.p, 1.4f);
    }
    if (layout) rt_->DrawTextLayout(D2D1::Point2F(rowX + box + gap, rowY + box / 2 - 12.0f), layout.p, ink.p);
    rcCheck_ = ToDevice(rowX - 6.0f, rowY - 6.0f, box + gap + labelW + 12.0f, box + 12.0f);
  };
  auto bodyLine = [&](const wchar_t* body) {
    if (fmtButton_) rt_->DrawTextW(body, (UINT32)wcslen(body), fmtButton_.p,
                                   D2D1::RectF(60, H * 0.53f, W - 60, H * 0.59f), muted.p);
  };
  auto twoPills = [&](const wchar_t* primary, const wchar_t* secondary) {
    const float ux = W / 2.0f - pw - 8.0f, cx = W / 2.0f + 8.0f;
    rcRetry_ = ToDevice(ux, py, pw, ph);
    rcErrClose_ = ToDevice(cx, py, pw, ph);
    pill(ux, py, pw, ph, primary, true);
    pill(cx, py, pw, ph, secondary, false);
  };

  if (stage_ == Stage::Welcome) {
    bodyLine(T(L"Installs for this Windows account. No administrator rights needed.",
               L"이 Windows 계정에 설치해요. 관리자 권한은 필요 없어요."));
    if (codePresent_) {
      // Nothing to offer: say what is there instead of a box that would do nothing.
      const wchar_t* have = T(L"Redrob Code is already installed on this PC.", L"Redrob Code가 이미 설치되어 있어요.");
      if (fmtButton_) rt_->DrawTextW(have, (UINT32)wcslen(have), fmtButton_.p,
                                     D2D1::RectF(60, H * 0.605f, W - 60, H * 0.665f), muted.p);
      rcCheck_ = RECT{};
    } else {
      checkRow(T(L"Also install Redrob Code, the agent that draws for you", L"Redrob Code도 설치하기 (도면을 그려 주는 에이전트)"), installCode_);
    }
    twoPills(T(L"Install", L"설치"), T(L"Cancel", L"취소"));
  } else if (stage_ == Stage::CodeFailed) {
    const std::wstring msg = g_ko ? L"Redrob Code는 설치하지 못했어요(" + codeError_ +
                                        L"). 나중에 github.com/redrob-labs/redrob-code에서 받을 수 있어요."
                                  : L"Redrob Code was not installed: " + codeError_ +
                                        L". Get it later from github.com/redrob-labs/redrob-code.";
    if (fmtButton_) rt_->DrawTextW(msg.c_str(), (UINT32)msg.size(), fmtButton_.p,
                                   D2D1::RectF(60, H * 0.53f, W - 60, H * 0.66f), muted.p);
    const float x = (W - pw) / 2.0f;
    rcRetry_ = ToDevice(x, py, pw, ph);  // Close
    rcErrClose_ = RECT{};
    pill(x, py, pw, ph, T(L"Close", L"닫기"), true);
  } else if (stage_ == Stage::Confirm) {
    bodyLine(T(L"Redrob CAD will be removed from this PC. Your drawings stay where they are.",
               L"이 PC에서 Redrob CAD를 제거해요. 도면 파일은 그대로 남아요."));
    checkRow(T(L"Also delete your settings and workspace key", L"설정과 워크스페이스 키도 삭제하기"), deleteData_);
    twoPills(T(L"Uninstall", L"제거"), T(L"Cancel", L"취소"));
  } else if (stage_ == Stage::Removed) {
    const wchar_t* body = deleteData_ ? T(L"Your settings and workspace key were deleted too.",
                                          L"설정과 워크스페이스 키도 삭제했어요.")
                                      : T(L"Your settings stay on this PC.", L"설정은 이 PC에 남아 있어요.");
    if (fmtButton_) rt_->DrawTextW(body, (UINT32)wcslen(body), fmtButton_.p,
                                   D2D1::RectF(60, H * 0.53f, W - 60, H * 0.59f), muted.p);
    const float x = (W - pw) / 2.0f;
    rcRetry_ = ToDevice(x, py, pw, ph);  // Close
    rcErrClose_ = RECT{};
    pill(x, py, pw, ph, T(L"Close", L"닫기"), true);
  } else if (stage_ != Stage::Error) {
    // ---- progress bar: thin, rounded, blue fill on a faint track -----------
    const float barW = W * 0.76f, barH = 6.0f;
    const float barX = (W - barW) / 2.0f, barY = H * 0.60f;
    D2D1_ROUNDED_RECT trackRR = {D2D1::RectF(barX, barY, barX + barW, barY + barH), barH / 2, barH / 2};
    rt_->FillRoundedRectangle(trackRR, track.p);
    float fillW = (float)(barW * progress_);
    if (fillW > barH) {
      D2D1_ROUNDED_RECT fillRR = {D2D1::RectF(barX, barY, barX + fillW, barY + barH), barH / 2, barH / 2};
      rt_->FillRoundedRectangle(fillRR, blue.p);
    }
  } else {
    // ---- error state: message + Retry (white pill, black type) / Close -----
    D2D1_RECT_F msgRc = D2D1::RectF(60, H * 0.54f, W - 60, H * 0.66f);
    if (fmtButton_) rt_->DrawTextW(errorMessage_.c_str(), (UINT32)errorMessage_.size(),
                                   fmtButton_.p, msgRc, muted.p);
    // Retry pill (white fill, black type) -- the single prominent CTA.
    const float pw = 120.0f, ph = 38.0f, py = H * 0.72f;
    const float rx = W / 2.0f - pw - 8.0f, cx = W / 2.0f + 8.0f;
    rcRetry_ = ToDevice(rx, py, pw, ph);
    rcErrClose_ = ToDevice(cx, py, pw, ph);
    D2D1_ROUNDED_RECT pill = {D2D1::RectF(rx, py, rx + pw, py + ph), ph / 2, ph / 2};
    rt_->FillRoundedRectangle(pill, white.p);
    D2D1_RECT_F rtxt = D2D1::RectF(rx, py, rx + pw, py + ph);
    { const wchar_t* l = T(L"Retry", L"다시 시도"); if (fmtButton_) rt_->DrawTextW(l, (UINT32)wcslen(l), fmtButton_.p, rtxt, black.p); }
    // Close (outline)
    D2D1_ROUNDED_RECT closeRR = {D2D1::RectF(cx, py, cx + pw, py + ph), ph / 2, ph / 2};
    rt_->DrawRoundedRectangle(closeRR, track.p, 1.4f);
    D2D1_RECT_F ctxt = D2D1::RectF(cx, py, cx + pw, py + ph);
    { const wchar_t* l = T(L"Close", L"닫기"); if (fmtButton_) rt_->DrawTextW(l, (UINT32)wcslen(l), fmtButton_.p, ctxt, ink.p); }
  }

  HRESULT hr = rt_->EndDraw();
  if (hr == D2DERR_RECREATE_TARGET) DiscardDeviceResources();
  EndPaint(hwnd_, &ps);
}

// device-pixel rect from logical (dip) coordinates
RECT SetupApp::ToDevice(float x, float y, float w, float h) {
  float s = dpi_ / 96.0f;
  RECT r;
  r.left = (LONG)(x * s); r.top = (LONG)(y * s);
  r.right = (LONG)((x + w) * s); r.bottom = (LONG)((y + h) * s);
  return r;
}

void SetupApp::computeButtonRects(float minX, float closeX, float cy) {
  float s = dpi_ / 96.0f;
  auto box = [&](float cx) {
    RECT r; r.left = (LONG)((cx - 16) * s); r.top = (LONG)((cy - 16) * s);
    r.right = (LONG)((cx + 16) * s); r.bottom = (LONG)((cy + 16) * s); return r;
  };
  rcMin_ = box(minX);
  rcClose_ = box(closeX);
}

// ---- payload ---------------------------------------------------------------

DWORD WINAPI SetupApp::PayloadThunk(LPVOID self) {
  return reinterpret_cast<SetupApp*>(self)->RunPayload();
}

// The embedded MSI, written to %TEMP% with an .msi name (msiexec goes by the extension).
bool SetupApp::ExtractPayload(std::wstring& outPath) {
  HRSRC res = FindResourceW(hinst_, MAKEINTRESOURCEW(IDR_PAYLOAD), RT_RCDATA);
  if (!res) return false;
  HGLOBAL h = LoadResource(hinst_, res);
  void* data = LockResource(h);
  DWORD sz = SizeofResource(hinst_, res);
  if (!data || !sz) return false;

  wchar_t tmp[MAX_PATH];
  GetTempPathW(MAX_PATH, tmp);
  wchar_t name[MAX_PATH];
  GetTempFileNameW(tmp, L"rcs", 0, name);
  std::wstring msi = std::wstring(name) + L".msi";
  HANDLE f = CreateFileW(msi.c_str(), GENERIC_WRITE, 0, nullptr, CREATE_ALWAYS,
                         FILE_ATTRIBUTE_NORMAL, nullptr);
  if (f == INVALID_HANDLE_VALUE) return false;
  DWORD written = 0;
  WriteFile(f, data, sz, &written, nullptr);
  CloseHandle(f);
  DeleteFileW(name);  // the 0-byte temp GetTempFileName created
  if (written != sz) return false;
  outPath = msi;
  return true;
}

// %LOCALAPPDATA% / %APPDATA% / %TEMP%, or empty.
static std::wstring KnownFolder(int csidl) {
  wchar_t dir[MAX_PATH];
  if (FAILED(SHGetFolderPathW(nullptr, csidl, nullptr, 0, dir))) return L"";
  return dir;
}
static std::wstring TempDir() {
  wchar_t tmp[MAX_PATH];
  DWORD n = GetTempPathW(MAX_PATH, tmp);
  return n ? std::wstring(tmp, n) : L"";
}

// Whether any redrob-cad.exe is running. Installing or removing over a running copy leaves files in use
// (msiexec would then want a reboot), and closing it for the person could lose a drawing, so we ask.
static bool AppRunning() {
  HANDLE snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
  if (snap == INVALID_HANDLE_VALUE) return false;
  bool found = false;
  PROCESSENTRY32W pe = {sizeof(pe)};
  for (BOOL ok = Process32FirstW(snap, &pe); ok && !found; ok = Process32NextW(snap, &pe))
    found = lstrcmpiW(pe.szExeFile, L"redrob-cad.exe") == 0;
  CloseHandle(snap);
  return found;
}

// Runs a command hidden and returns its exit code. `elevate` goes through ShellExecuteEx "runas" (one
// UAC prompt), for a per-machine install made by an administrator with the MSI directly.
static DWORD RunHidden(const std::wstring& exe, const std::wstring& args, bool elevate) {
  if (elevate) {
    SHELLEXECUTEINFOW sei = {sizeof(sei)};
    sei.fMask = SEE_MASK_NOCLOSEPROCESS | SEE_MASK_NOASYNC;
    sei.lpVerb = L"runas";
    sei.lpFile = exe.c_str();
    sei.lpParameters = args.c_str();
    sei.nShow = SW_HIDE;
    if (!ShellExecuteExW(&sei) || !sei.hProcess) return GetLastError() ? GetLastError() : 1;
    WaitForSingleObject(sei.hProcess, INFINITE);
    DWORD code = 1;
    GetExitCodeProcess(sei.hProcess, &code);
    CloseHandle(sei.hProcess);
    return code;
  }
  std::wstring cmd = L"\"" + exe + L"\" " + args;
  STARTUPINFOW si = {sizeof(si)};
  si.dwFlags = STARTF_USESHOWWINDOW;
  si.wShowWindow = SW_HIDE;
  PROCESS_INFORMATION pi = {};
  DWORD code = ERROR_FILE_NOT_FOUND;
  if (CreateProcessW(nullptr, &cmd[0], nullptr, nullptr, FALSE, CREATE_NO_WINDOW, nullptr, nullptr, &si, &pi)) {
    WaitForSingleObject(pi.hProcess, INFINITE);
    GetExitCodeProcess(pi.hProcess, &code);
    CloseHandle(pi.hThread);
    CloseHandle(pi.hProcess);
  }
  return code;
}

static std::wstring Msiexec() {
  wchar_t sys[MAX_PATH];
  GetSystemDirectoryW(sys, MAX_PATH);
  return std::wstring(sys) + L"\\msiexec.exe";
}

// Where the MSI put redrob-cad.exe: its App Paths entry (written under HKMU, so HKCU for a per-user
// install and HKLM for a per-machine one).
static std::wstring InstalledExe() {
  const wchar_t* key = L"Software\\Microsoft\\Windows\\CurrentVersion\\App Paths\\redrob-cad.exe";
  for (HKEY root : {HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE}) {
    wchar_t buf[MAX_PATH * 2];
    DWORD size = sizeof(buf);
    if (RegGetValueW(root, key, nullptr, RRF_RT_REG_SZ, nullptr, buf, &size) == ERROR_SUCCESS &&
        GetFileAttributesW(buf) != INVALID_FILE_ATTRIBUTES)
      return buf;
  }
  return L"";
}

// Start the installed Redrob CAD, unless one is already running (an update leaves nothing open,
// so this is the only launch there is).
void SetupApp::LaunchApp() {
  if (AppRunning()) return;
  const std::wstring exe = InstalledExe();
  if (exe.empty()) return;
  const std::wstring dir = exe.substr(0, exe.find_last_of(L'\\'));
  ShellExecuteW(nullptr, L"open", exe.c_str(), nullptr, dir.c_str(), SW_SHOWNORMAL);
}

// The installed product, found by UpgradeCode (the ProductCode changes every version). Also whether it
// is a per-machine install, which needs elevation to remove.
static std::wstring InstalledProduct(bool* machine) {
  wchar_t code[39];
  if (MsiEnumRelatedProductsW(kUpgradeCode, 0, 0, code) != ERROR_SUCCESS) return L"";
  wchar_t buf[8];
  DWORD n = 8;
  *machine = MsiGetProductInfoExW(code, nullptr, MSIINSTALLCONTEXT_MACHINE, INSTALLPROPERTY_PRODUCTSTATE, buf, &n) ==
             ERROR_SUCCESS;
  return code;
}

DWORD WINAPI SetupApp::UninstallThunk(LPVOID self) {
  return reinterpret_cast<SetupApp*>(self)->RunUninstall();
}

void SetupApp::StartUninstall() {
  progress_ = progressTarget_ = 0.0;
  startTick_ = GetTickCount64();
  SetStage(Stage::Removing);
  SetTimer(hwnd_, kTickTimer, kTickMs, nullptr);
  if (payloadThread_) { WaitForSingleObject(payloadThread_, 5000); CloseHandle(payloadThread_); }
  payloadThread_ = CreateThread(nullptr, 0, &SetupApp::UninstallThunk, this, 0, nullptr);
}

// Deletes a folder and everything in it (the settings folder; nothing else is ever passed here).
static void DeleteTree(const std::wstring& dir) {
  if (dir.size() < 8) return;
  std::wstring from = dir + L'\0';  // SHFileOperation wants a double-NUL-terminated list
  SHFILEOPSTRUCTW op = {};
  op.wFunc = FO_DELETE;
  op.pFrom = from.c_str();
  op.fFlags = FOF_NO_UI;
  SHFileOperationW(&op);
}

// msiexec /x removes the files, shortcuts and the Settings > Apps entry. The settings folder is the app's
// own (apps/cadcraft/src/device.rs config_dir), so only the opt-in deletes it.
DWORD SetupApp::RunUninstall() {
  if (demo_) {
    Sleep(4500);
    PostMessage(hwnd_, kUninstallMsgDone, 0, 0);
    return 0;
  }
  if (AppRunning()) {
    PostMessage(hwnd_, kUninstallMsgDone, (WPARAM)kAppRunning, 0);
    return 1;
  }
  bool machine = false;
  const std::wstring product = InstalledProduct(&machine);
  DWORD code = kMsiUnknownProduct;
  if (!product.empty()) code = RunHidden(Msiexec(), L"/x " + product + L" /qn /norestart", machine);
  if ((code == 0 || code == kMsiRebootRequired || code == kMsiUnknownProduct) && deleteData_) {
    const std::wstring appdata = KnownFolder(CSIDL_APPDATA);
    if (!appdata.empty()) DeleteTree(appdata + L"\\Redrob CAD");
  }
  PostMessage(hwnd_, kUninstallMsgDone, (WPARAM)code, 0);
  return code;
}

// Settings > Apps starts redrob-cad-uninstall.exe from the install folder, which msiexec has to delete.
// So the first start copies itself to %TEMP% and hands over; the copy runs the window.
static bool RelaunchFromTemp() {
  wchar_t self[MAX_PATH];
  if (!GetModuleFileNameW(nullptr, self, MAX_PATH)) return false;
  const std::wstring tmp = TempDir();
  if (tmp.empty() || _wcsnicmp(self, tmp.c_str(), tmp.size()) == 0) return false;  // already the copy
  const std::wstring copy = tmp + L"redrob-cad-uninstall-" + std::to_wstring(GetCurrentProcessId()) + L".exe";
  if (!CopyFileW(self, copy.c_str(), FALSE)) return false;
  std::wstring cmd = L"\"" + copy + L"\" --uninstall";
  if (g_ko) cmd += L" --lang ko";
  STARTUPINFOW si = {sizeof(si)};
  PROCESS_INFORMATION pi = {};
  if (!CreateProcessW(nullptr, &cmd[0], nullptr, nullptr, FALSE, 0, nullptr, tmp.c_str(), &si, &pi)) {
    DeleteFileW(copy.c_str());
    return false;
  }
  CloseHandle(pi.hThread);
  CloseHandle(pi.hProcess);
  return true;
}

// Once removed, the temp copy is the last thing of ours left. A running exe cannot delete itself, so a
// hidden PowerShell waits for this process to exit and removes it. Only a copy in %TEMP% is ever deleted.
void SetupApp::ScheduleSelfCleanup() {
  wchar_t self[MAX_PATH];
  if (!GetModuleFileNameW(nullptr, self, MAX_PATH)) return;
  const std::wstring tmp = TempDir();
  std::wstring me = self;
  if (tmp.size() < 4 || _wcsnicmp(me.c_str(), tmp.c_str(), tmp.size()) != 0) return;
  std::wstring cmd = L"powershell.exe -NoProfile -NonInteractive -WindowStyle Hidden -Command \"Wait-Process -Id " +
                     std::to_wstring(GetCurrentProcessId()) +
                     L" -ErrorAction SilentlyContinue; Remove-Item -LiteralPath '" + me +
                     L"' -Force -ErrorAction SilentlyContinue\"";
  STARTUPINFOW si = {sizeof(si)};
  si.dwFlags = STARTF_USESHOWWINDOW;
  si.wShowWindow = SW_HIDE;
  PROCESS_INFORMATION pi = {};
  if (CreateProcessW(nullptr, &cmd[0], nullptr, nullptr, FALSE, CREATE_NO_WINDOW, nullptr, nullptr, &si, &pi)) {
    CloseHandle(pi.hThread);
    CloseHandle(pi.hProcess);
  }
}

// ---- Redrob Code (opt-in) --------------------------------------------------

// Lower-case hex SHA-256 of a file, or empty.
static std::string Sha256File(const std::wstring& path) {
  std::string out;
  HANDLE f = CreateFileW(path.c_str(), GENERIC_READ, FILE_SHARE_READ, nullptr, OPEN_EXISTING, 0, nullptr);
  if (f == INVALID_HANDLE_VALUE) return out;
  BCRYPT_ALG_HANDLE alg = nullptr;
  BCRYPT_HASH_HANDLE hash = nullptr;
  if (BCRYPT_SUCCESS(BCryptOpenAlgorithmProvider(&alg, BCRYPT_SHA256_ALGORITHM, nullptr, 0)) &&
      BCRYPT_SUCCESS(BCryptCreateHash(alg, &hash, nullptr, 0, nullptr, 0, 0))) {
    std::vector<unsigned char> buf(1 << 20);
    DWORD got = 0;
    bool ok = true;
    while (ReadFile(f, buf.data(), (DWORD)buf.size(), &got, nullptr) && got > 0) {
      if (!BCRYPT_SUCCESS(BCryptHashData(hash, buf.data(), got, 0))) { ok = false; break; }
    }
    unsigned char digest[32];
    if (ok && BCRYPT_SUCCESS(BCryptFinishHash(hash, digest, sizeof(digest), 0))) {
      static const char hex[] = "0123456789abcdef";
      for (unsigned char b : digest) { out += hex[b >> 4]; out += hex[b & 15]; }
    }
  }
  if (hash) BCryptDestroyHash(hash);
  if (alg) BCryptCloseAlgorithmProvider(alg, 0);
  CloseHandle(f);
  return out;
}

// The hash SHA256SUMS lists for `asset` ("<64 hex>  <name>" per line, as `sha256sum` writes), or empty.
static std::string ExpectedSha256(const std::wstring& sumsPath, const char* asset) {
  HANDLE f = CreateFileW(sumsPath.c_str(), GENERIC_READ, FILE_SHARE_READ, nullptr, OPEN_EXISTING, 0, nullptr);
  if (f == INVALID_HANDLE_VALUE) return "";
  std::string text(64 * 1024, '\0');
  DWORD got = 0;
  ReadFile(f, &text[0], (DWORD)text.size(), &got, nullptr);
  CloseHandle(f);
  text.resize(got);
  size_t start = 0;
  while (start < text.size()) {
    size_t end = text.find('\n', start);
    if (end == std::string::npos) end = text.size();
    std::string line = text.substr(start, end - start);
    if (!line.empty() && line.back() == '\r') line.pop_back();
    start = end + 1;
    // Exact name after the two-space separator (or "*" for binary mode), so a longer asset name that merely
    // ends the same way never matches.
    if (line.size() < 66 || (line.compare(64, 2, "  ") != 0 && line.compare(64, 2, " *") != 0)) continue;
    if (line.compare(66, std::string::npos, asset) != 0) continue;
    std::string h = line.substr(0, 64);
    std::transform(h.begin(), h.end(), h.begin(), [](char c) { return (char)tolower((unsigned char)c); });
    if (h.find_first_not_of("0123456789abcdef") != std::string::npos) return "";
    return h;
  }
  return "";
}

static bool Download(const wchar_t* url, const std::wstring& to) {
  DeleteUrlCacheEntryW(url);  // `latest` moves; never install a cached older redirect
  return SUCCEEDED(URLDownloadToFileW(nullptr, url, to.c_str(), 0, nullptr)) &&
         GetFileAttributesW(to.c_str()) != INVALID_FILE_ATTRIBUTES;
}

// Download the latest Redrob Code installer, refuse it unless it matches its release's SHA256SUMS, and run it
// silently for this user only. Inno Setup exits 0 on success.
std::wstring SetupApp::InstallRedrobCode() {
  wchar_t tmp[MAX_PATH];
  GetTempPathW(MAX_PATH, tmp);
  const std::wstring dir = std::wstring(tmp) + L"redrob-code-setup-" + std::to_wstring(GetCurrentProcessId());
  CreateDirectoryW(dir.c_str(), nullptr);
  const std::wstring sums = dir + L"\\SHA256SUMS";
  const std::wstring exe = dir + L"\\redrob-code-x64-setup.exe";
  auto cleanup = [&] {
    DeleteFileW(sums.c_str());
    DeleteFileW(exe.c_str());
    RemoveDirectoryW(dir.c_str());
  };

  if (!Download(kCodeSumsUrl, sums) || !Download(kCodeSetupUrl, exe)) {
    cleanup();
    return L"it could not be downloaded";
  }
  const std::string want = ExpectedSha256(sums, kCodeSetupAsset);
  const std::string got = Sha256File(exe);
  if (want.empty() || got != want) {
    cleanup();
    return L"its download did not match the release checksum";
  }

  std::wstring cmd = L"\"" + exe + L"\" /VERYSILENT /SUPPRESSMSGBOXES /NORESTART /SP- /CURRENTUSER /TASKS=addtopath";
  STARTUPINFOW si = {sizeof(si)};
  si.dwFlags = STARTF_USESHOWWINDOW;
  si.wShowWindow = SW_HIDE;
  PROCESS_INFORMATION pi = {};
  DWORD exitCode = ERROR_FILE_NOT_FOUND;
  if (CreateProcessW(nullptr, &cmd[0], nullptr, nullptr, FALSE, CREATE_NO_WINDOW, nullptr, dir.c_str(), &si, &pi)) {
    WaitForSingleObject(pi.hProcess, INFINITE);
    GetExitCodeProcess(pi.hProcess, &exitCode);
    CloseHandle(pi.hThread);
    CloseHandle(pi.hProcess);
  }
  cleanup();
  if (exitCode != 0) return L"its installer stopped (code " + std::to_wstring(exitCode) + L")";
  return L"";
}

DWORD SetupApp::RunPayload() {
  // Redrob Code first, when chosen, so Redrob CAD opens already able to find its agent. Its failure is
  // reported at the end and never stops the Redrob CAD install.
  if (installCode_ && !codeDone_) {
    PostMessage(hwnd_, kStageMsg, (WPARAM)Stage::InstallingCode, 0);
    codeError_ = InstallRedrobCode();
    codeDone_ = codeError_.empty();
  }
  PostMessage(hwnd_, kStageMsg, (WPARAM)Stage::Installing, 0);

  if (AppRunning()) {
    PostMessage(hwnd_, kPayloadMsgDone, (WPARAM)kAppRunning, 0);
    return 1;
  }
  std::wstring msi;
  if (!ExtractPayload(msi)) {
    PostMessage(hwnd_, kPayloadMsgDone, (WPARAM)ERROR_RESOURCE_DATA_NOT_FOUND, 0);
    return 1;
  }
  extractedPayload_ = msi;

  // Per-user (ALLUSERS=2 + MSIINSTALLPERUSER=1): %LOCALAPPDATA%\Programs\Redrob CAD, HKCU, no UAC. The
  // log stays in %TEMP% for a failed run to be diagnosed from.
  const std::wstring log = TempDir() + L"redrob-cad-install.log";
  const DWORD exitCode =
      RunHidden(Msiexec(), L"/i \"" + msi + L"\" /qn /norestart ALLUSERS=2 MSIINSTALLPERUSER=1 /l*v \"" + log + L"\"", false);
  DeleteFileW(msi.c_str());  // tidy the temp payload
  if (exitCode == 0 || exitCode == kMsiRebootRequired) LaunchApp();
  PostMessage(hwnd_, kPayloadMsgDone, (WPARAM)exitCode, 0);
  return exitCode;
}

}  // namespace

// ---- entry point -----------------------------------------------------------

int WINAPI wWinMain(HINSTANCE hinst, HINSTANCE, PWSTR, int) {
  bool demo = false, demoError = false, withCode = false, demoCodeError = false;
  Mode mode = Mode::Install;
  std::wstring lang;
  int argc = 0;
  LPWSTR* argv = CommandLineToArgvW(GetCommandLineW(), &argc);
  for (int i = 1; i < argc; ++i) {
    if (lstrcmpiW(argv[i], L"--demo") == 0) demo = true;
    else if (lstrcmpiW(argv[i], L"--demo-error") == 0) { demo = true; demoError = true; }
    else if (lstrcmpiW(argv[i], L"--demo-code-error") == 0) { demo = true; demoCodeError = true; }
    else if (lstrcmpiW(argv[i], L"--with-redrob-code") == 0) withCode = true;
    else if (lstrcmpiW(argv[i], L"--uninstall") == 0) mode = Mode::Uninstall;
    else if (lstrcmpiW(argv[i], L"--demo-uninstall") == 0) { demo = true; mode = Mode::Uninstall; }
    else if (lstrcmpiW(argv[i], L"--lang") == 0 && i + 1 < argc) { lang = argv[++i]; }
  }
  if (argv) LocalFree(argv);
  g_ko = lang.empty() ? PRIMARYLANGID(GetUserDefaultUILanguage()) == LANG_KOREAN : _wcsicmp(lang.c_str(), L"ko") == 0;
  // The uninstaller lives in the install folder msiexec is about to delete: run from a temp copy.
  if (mode == Mode::Uninstall && !demo && RelaunchFromTemp()) return 0;

  SetupApp app(demo, demoError, mode, withCode, demoCodeError);
  return app.Run(hinst);
}
