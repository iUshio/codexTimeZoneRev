import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:shared_preferences/shared_preferences.dart';
import 'package:timezone/data/latest.dart' as timezone;
import 'package:codex_timezone/app/application.dart';
import 'package:codex_timezone/services/backend.dart';
import 'package:codex_timezone/services/desktop_services.dart';
import 'package:codex_timezone/services/network_info.dart';

class FakeDesktop implements DesktopServices {
  String? selected;
  String? copied;
  bool failCopy = false;
  @override
  Future<String?> chooseClient(String platform) async => selected;
  @override
  Future<void> copyVerified(String text) async {
    if (failCopy) throw StateError('clipboard unavailable');
    copied = text;
  }
}

class Backend implements LauncherBackend {
  final List<String> calls = [];
  bool repairRequired = false;
  @override
  Future<Map<String, dynamic>> call(
    String command, [
    Map<String, dynamic> payload = const {},
  ]) async {
    calls.add(command);
    if (command == 'bootstrap') return const PreviewBackend().call(command);
    if (command == 'validate') return {'path': payload['path']};
    if (command == 'launch' && repairRequired) {
      return {
        'repairRequired': true,
        'target': {
          'executable': r'C:\WindowsApps\current\app\ChatGPT.exe',
          'package': {
            'fullName': 'OpenAI.Codex_26.928.2636.0_x64__2p2nqsd0c76g0',
          },
        },
        'message': '检测到包身份异常。',
      };
    }
    if (command == 'repair_launch') {
      return {'cancelled': true, 'message': '已取消管理员确认'};
    }
    return {'message': '设置已保存。'};
  }
}

void main() {
  setUpAll(timezone.initializeTimeZones);
  for (final width in [600.0, 1280.0]) {
    testWidgets('repair action is explicit and usable at width $width', (
      tester,
    ) async {
      SharedPreferences.setMockInitialValues({});
      tester.view.devicePixelRatio = 1;
      tester.view.physicalSize = Size(width, 800);
      addTearDown(tester.view.resetDevicePixelRatio);
      addTearDown(tester.view.resetPhysicalSize);
      final backend = Backend()..repairRequired = true;
      await tester.pumpWidget(
        LauncherApplication(
          backend: backend,
          networkFactory: () => NetworkController(preview: true),
        ),
      );
      await tester.pumpAndSettle();
      expect(find.text('一键修复并重试'), findsNothing);
      await tester.tap(find.text('保存并启动'));
      await tester.pumpAndSettle();
      expect(backend.calls, ['bootstrap', 'launch']);
      expect(find.text('一键修复并重试'), findsOneWidget);
      expect(find.textContaining('需要 Windows 管理员确认'), findsOneWidget);
      await tester.tap(find.text('一键修复并重试'));
      await tester.pumpAndSettle();
      expect(backend.calls, ['bootstrap', 'launch', 'repair_launch']);
      expect(find.text('已取消管理员确认'), findsOneWidget);
      expect(tester.takeException(), isNull);
      await tester.pumpWidget(const SizedBox());
    });
  }
  testWidgets(
    'Forui file selection, dirty state and clipboard feedback are wired',
    (tester) async {
      SharedPreferences.setMockInitialValues({});
      tester.view.devicePixelRatio = 1;
      tester.view.physicalSize = const Size(1280, 900);
      addTearDown(tester.view.resetDevicePixelRatio);
      addTearDown(tester.view.resetPhysicalSize);
      final desktop = FakeDesktop()..selected = r'C:\测试 客户端\Codex.exe';
      final backend = Backend();
      await tester.pumpWidget(
        LauncherApplication(
          backend: backend,
          desktop: desktop,
          networkFactory: () => NetworkController(preview: true),
        ),
      );
      await tester.pumpAndSettle();
      await tester.tap(find.text('选择客户端'));
      await tester.pumpAndSettle();
      expect(backend.calls, ['bootstrap', 'validate']);
      expect(find.text('未保存修改'), findsOneWidget);
      expect(find.text(r'C:\测试 客户端\Codex.exe'), findsOneWidget);
      await tester.tap(find.text('保存设置'));
      await tester.pumpAndSettle();
      expect(find.text('未保存修改'), findsNothing);
      await tester.ensureVisible(find.text('复制 IP').first);
      await tester.pumpAndSettle();
      await tester.tap(find.text('复制 IP').first);
      await tester.pumpAndSettle();
      expect(desktop.copied, '192.0.2.10');
      expect(find.text('已复制'), findsOneWidget);
      await tester.pump(const Duration(milliseconds: 1600));
      desktop.failCopy = true;
      await tester.tap(find.text('复制 IP').first);
      await tester.pumpAndSettle();
      expect(find.text('复制失败'), findsOneWidget);
      expect(find.text('无法复制 IP。'), findsWidgets);
      await tester.ensureVisible(find.text('重新注入皮肤'));
      await tester.pumpAndSettle();
      await tester.tap(find.text('重新注入皮肤'));
      await tester.pumpAndSettle();
      expect(backend.calls.last, 'reapply_dream_skin');
      expect(tester.takeException(), isNull);
      await tester.pumpWidget(const SizedBox());
    },
  );
}
