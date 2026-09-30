import 'package:flutter/foundation.dart';

import '../domain/settings.dart';
import '../services/backend.dart';

class LauncherController extends ChangeNotifier {
  LauncherController(this.backend, {this.preview = false});
  final LauncherBackend backend;
  final bool preview;
  LauncherSettings settings = const LauncherSettings();
  LauncherSettings? _saved;
  List<Zone> zones = [];
  String detected = '';
  String platform = 'windows';
  String message = '正在读取设置并查找 Codex…';
  String? error;
  String? busyCommand;
  bool initializing = true;
  bool bootstrapFailed = false;
  bool _disposed = false;
  bool _loading = false;
  String? repairPackage;
  String? repairLogPath;

  bool get busy => busyCommand != null;
  bool get editable => !initializing && !bootstrapFailed && !busy;
  bool get dirty => _saved != null && settings != _saved;
  String get effectivePath => settings.executable.trim().isEmpty
      ? detected
      : settings.executable.trim();
  String? get validationError {
    if (settings.mode == 'zone') {
      return zones.any((zone) => zone.id == settings.zoneId)
          ? null
          : '请选择支持的地区时区。';
    }
    if (settings.mode != 'offset' ||
        settings.offset < -12 ||
        settings.offset > 14) {
      return '请选择 −12 至 +14 小时的 UTC 偏移。';
    }
    return null;
  }

  bool get canSave => editable && validationError == null && !preview;
  bool get canLaunch => canSave && effectivePath.isNotEmpty;

  void _emit() {
    if (!_disposed) notifyListeners();
  }

  void update(LauncherSettings next) {
    if (!editable) return;
    settings = next;
    repairPackage = null;
    repairLogPath = null;
    _emit();
  }

  void report(String summary, {String? detail}) {
    message = summary;
    error = detail;
    _emit();
  }

  Future<void> initialize() async {
    if (_loading || busy) return;
    _loading = true;
    initializing = true;
    bootstrapFailed = false;
    report('正在读取设置并查找 Codex…');
    try {
      final data = await backend.call('bootstrap');
      final loaded = LauncherSettings.fromJson(
        data['settings'] as Map<String, dynamic>,
      );
      final loadedZones = (data['zones'] as List)
          .map((value) => Zone.fromJson(value as Map<String, dynamic>))
          .toList();
      settings = loaded;
      zones = loadedZones;
      _saved = loaded;
      detected = data['detected'] as String? ?? '';
      platform = data['platform'] as String? ?? 'windows';
      message = preview
          ? '演示模式：系统操作已禁用。'
          : detected.isEmpty
          ? '设置已读取，请选择 Codex 客户端。'
          : '设置已读取，客户端已自动找到。';
    } catch (e) {
      bootstrapFailed = true;
      message = '读取设置失败，请重试。';
      error = e.toString();
    } finally {
      initializing = false;
      _loading = false;
      _emit();
    }
  }

  Future<void> browse(Future<String?> Function() choosePath) async {
    if (!editable || preview) return;
    busyCommand = 'browse';
    _emit();
    try {
      final path = await choosePath();
      if (path == null || _disposed) return;
      final data = await backend.call('validate', {'path': path});
      settings = settings.copyWith(executable: data['path'] as String);
      repairPackage = null;
      report('客户端路径已更新，尚未保存。');
    } catch (e) {
      report('无法使用所选客户端路径。', detail: e.toString());
    } finally {
      busyCommand = null;
      _emit();
    }
  }

  Future<bool> action(String command) async {
    if (!editable || preview) return false;
    if (![
      'save',
      'launch',
      'repair_launch',
      'create_shortcut',
      'launch_dream_skin',
      'reapply_dream_skin',
    ].contains(command)) {
      return false;
    }
    if (['save', 'launch', 'repair_launch'].contains(command) &&
        validationError != null) {
      report('时区设置无效。', detail: validationError);
      return false;
    }
    if (command == 'launch' && effectivePath.isEmpty) {
      report('未找到 Codex，请选择程序路径。');
      return false;
    }
    final submitted = settings;
    final expectedPackage = repairPackage;
    if (command == 'repair_launch' && expectedPackage == null) return false;
    if (command == 'launch') repairPackage = null;
    busyCommand = command;
    report(switch (command) {
      'save' => '正在保存设置…',
      'launch' => '正在识别当前版本、检查身份并启动 Codex…',
      'repair_launch' => '正在检查并修复，请确认 Windows 管理员提示…',
      'create_shortcut' => '正在创建桌面快捷方式…',
      'launch_dream_skin' => '正在打开 Dream Skin…',
      _ => '正在重新注入 Dream Skin…',
    });
    try {
      var data = await backend.call(command, {
        'settings': submitted.toJson(),
        if (command == 'repair_launch') 'packageFullName': expectedPackage,
      });
      if (command == 'repair_launch') {
        if (data['cancelled'] == true) {
          report(data['message'] as String? ?? '已取消修复，未启动客户端。');
          return false;
        }
        if (data['healthy'] != true) throw StateError('修复后身份未通过，已停止启动。');
        if (_disposed) return false;
        report('身份检查通过，正在按所选时区重试启动…');
        // Exactly one retry, with fresh native discovery and identity checking.
        data = await backend.call('launch', {'settings': submitted.toJson()});
      }
      if (data['repairRequired'] == true) {
        final target = data['target'] as Map<String, dynamic>;
        final package = target['package'] as Map<String, dynamic>;
        repairPackage = package['fullName'] as String;
        detected = target['executable'] as String;
        repairLogPath = data['logPath'] as String?;
        report(data['message'] as String? ?? '当前客户端需要修复后才能启动。');
        return false;
      }
      if (command == 'save' ||
          command == 'launch' ||
          command == 'repair_launch') {
        final savedSettings = data['settings'];
        settings = savedSettings is Map<String, dynamic>
            ? LauncherSettings.fromJson(savedSettings)
            : submitted;
        _saved = settings;
        repairPackage = null;
        repairLogPath = null;
        final target = data['target'];
        if (target is Map<String, dynamic>) {
          detected = target['executable'] as String;
        }
      }
      report(data['message'] as String? ?? '操作已完成。');
      return true;
    } catch (e) {
      if (command == 'repair_launch') repairPackage = null;
      report('操作失败。', detail: e.toString());
      return false;
    } finally {
      busyCommand = null;
      _emit();
    }
  }

  @override
  void dispose() {
    _disposed = true;
    super.dispose();
  }
}
