import 'dart:async';
import 'dart:convert';
import 'dart:io';
import 'dart:ui' as ui;

import 'package:codex_timezone/app/application.dart';
import 'package:codex_timezone/domain/settings.dart';
import 'package:codex_timezone/services/account_usage.dart';
import 'package:codex_timezone/services/backend.dart';
import 'package:codex_timezone/services/clocks.dart';
import 'package:codex_timezone/services/network_info.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:forui/forui.dart';
import 'package:shared_preferences/shared_preferences.dart';
import 'package:timezone/data/latest.dart' as timezone;

import 'account_usage_test.dart' show usageResponse;

class PageBackend implements LauncherBackend {
  final List<String> calls = [];
  String? detected;
  @override
  Future<Map<String, dynamic>> call(
    String command, [
    Map<String, dynamic> payload = const {},
  ]) async {
    calls.add(command);
    if (command == 'bootstrap') {
      final data = await const PreviewBackend().call(command);
      if (detected != null) data['detected'] = detected;
      return data;
    }
    return {'message': '完成'};
  }
}

class PageUsage implements AccountUsageService {
  int calls = 0;
  String? selectedPath;
  Completer<AccountUsage>? pending;
  String? failure;
  Map<String, dynamic> response = usageResponse();
  @override
  Future<AccountUsage> query(String desktopPath) async {
    calls++;
    selectedPath = desktopPath;
    if (failure != null) throw StateError(failure!);
    if (pending != null) return pending!.future;
    return AccountUsage.fromJson(response);
  }
}

class LiveUsage implements AccountUsageService {
  int calls = 0;
  AccountUsage? result;
  @override
  Future<AccountUsage> query(String desktopPath) async {
    calls++;
    return result = await const NativeAccountUsageService(NativeBackend())
        .query(desktopPath);
  }
}

void main() {
  final capture = Platform.environment['CAPTURE_USAGE_UI_DIR'];
  setUpAll(() async {
    timezone.initializeTimeZones();
    if (capture == null) return;
    for (final (family, asset) in [
      ('packages/forui/Inter', 'packages/forui/assets/fonts/inter/Inter.ttf'),
      (
        'packages/forui_lucide/ForuiLucideIcons',
        'packages/forui_lucide/assets/lucide.ttf',
      ),
    ]) {
      await (FontLoader(family)..addFont(rootBundle.load(asset))).load();
    }
    final cjk = File(r'C:\Windows\Fonts\msyh.ttc');
    if (cjk.existsSync()) {
      await (FontLoader('Microsoft YaHei')..addFont(
            cjk.readAsBytes().then((bytes) => ByteData.sublistView(bytes)),
          ))
          .load();
    }
  });

  Future<void> screenshot(
    WidgetTester tester,
    GlobalKey key,
    String name,
  ) async {
    if (capture == null) return;
    await tester.runAsync(() async {
      final boundary =
          key.currentContext!.findRenderObject()! as RenderRepaintBoundary;
      final image = await boundary.toImage();
      try {
        final png = await image.toByteData(format: ui.ImageByteFormat.png);
        final directory = Directory(capture)..createSync(recursive: true);
        await File('${directory.path}/$name.png')
            .writeAsBytes(png!.buffer.asUint8List());
      } finally {
        image.dispose();
      }
    });
  }

  for (final dark in [false, true]) {
    for (final width in [600.0, 1280.0]) {
      testWidgets(
        'manual account panel displays and remains usable at $width / dark=$dark',
        (tester) async {
          SharedPreferences.setMockInitialValues({});
          tester.view.devicePixelRatio = 1;
          tester.view.physicalSize = Size(width, 1200);
          addTearDown(tester.view.resetDevicePixelRatio);
          addTearDown(tester.view.resetPhysicalSize);
          final service = PageUsage();
          final backend = PageBackend();
          final boundary = GlobalKey();
          await tester.pumpWidget(
            RepaintBoundary(
              key: boundary,
              child: LauncherApplication(
                backend: backend,
                usageService: service,
                networkFactory: () => NetworkController(preview: true),
                initialAppearance: dark ? 'dark' : 'light',
              ),
            ),
          );
          await tester.pumpAndSettle();
          expect(service.calls, 0);
          expect(find.text('点击按钮后查询，不会自动获取或刷新。'), findsOneWidget);
          expect(find.text('本地时间 · 系统时区'), findsOneWidget);
          final card = find.byKey(const ValueKey('account-usage-card'));
          expect(
            tester.getTopLeft(card).dy,
            greaterThan(tester.getTopLeft(find.text('时间预览')).dy),
          );
          if (width >= 820) {
            expect(
              tester.getTopLeft(card).dx,
              greaterThan(tester.getTopLeft(find.text('启动设置')).dx),
            );
            expect(
              tester.getRect(card).bottom,
              closeTo(
                tester
                    .getRect(find.byKey(const ValueKey('launch-settings-card')))
                    .bottom,
                0.1,
              ),
            );
            expect(
              tester
                  .getRect(find.byKey(const ValueKey('time-preview-card')))
                  .top,
              closeTo(
                tester
                    .getRect(find.byKey(const ValueKey('launch-settings-card')))
                    .top,
                0.1,
              ),
            );
            await screenshot(
              tester,
              boundary,
              '${dark ? 'dark' : 'light'}-idle',
            );
          }
          await tester.pump(const Duration(minutes: 2));
          expect(service.calls, 0);
          final button = find.byKey(const ValueKey('query-account-usage'));
          await tester.ensureVisible(button);
          await tester.tap(button);
          await tester.pumpAndSettle();
          expect(service.calls, 1);
          expect(service.selectedPath, contains('Codex.exe'));
          expect(find.text('5 小时额度'), findsOneWidget);
          expect(find.text('周额度'), findsOneWidget);
          expect(find.text('已用 8%'), findsOneWidget);
          expect(find.text('已用 5%'), findsOneWidget);
          expect(find.text('1,234,567'), findsOneWidget);
          expect(find.text('12,345'), findsOneWidget);
          // This page uses live clocks. A fixed calendar date makes the release
          // suite fail the day after it was authored; clock values have their
          // own deterministic tests in clocks_test.dart.
          expect(
            find.textContaining(
              RegExp(r'^\d{4} 年 \d{1,2} 月 \d{1,2} 日 · 星期[一二三四五六日]$'),
            ),
            findsNWidgets(2),
          );
          expect(find.byType(FDeterminateProgress), findsNWidgets(3));
          if (width >= 820) {
            expect(
              tester.getRect(card).bottom,
              closeTo(
                tester
                    .getRect(find.byKey(const ValueKey('launch-settings-card')))
                    .bottom,
                0.1,
              ),
            );
          }
          final reset = DateTime.fromMillisecondsSinceEpoch(
            1791981390000,
            isUtc: true,
          );
          expect(
            find.text(
              usageResetTime(const LauncherSettings(), reset, local: true),
            ),
            findsOneWidget,
          );
          await tester.ensureVisible(
            find.byKey(const ValueKey('usage-time-basis')),
          );
          await tester.tap(
            find.descendant(
              of: find.byKey(const ValueKey('usage-time-basis')),
              matching: find.text('本地时间'),
            ),
          );
          await tester.pumpAndSettle();
          await tester.tap(find.text('节点时间').last);
          await tester.pumpAndSettle();
          expect(
            find.text(
              usageResetTime(const LauncherSettings(), reset, local: false),
            ),
            findsOneWidget,
          );
          expect(service.calls, 1);
          if (width >= 820) {
            expect(
              tester.getRect(card).bottom,
              closeTo(
                tester
                    .getRect(find.byKey(const ValueKey('launch-settings-card')))
                    .bottom,
                0.1,
              ),
            );
          }
          expect(tester.takeException(), isNull);
          await tester.pump(const Duration(minutes: 2));
          expect(service.calls, 1);
          if (width >= 820) {
            await tester.drag(
              find.byType(SingleChildScrollView).first,
              const Offset(0, 2000),
            );
            await tester.pumpAndSettle();
            await screenshot(
              tester,
              boundary,
              '${dark ? 'dark' : 'light'}-loaded',
            );
          }
          await tester.ensureVisible(find.text('累计 token'));
          await tester.pumpAndSettle();
          expect(tester.takeException(), isNull);
          service.failure = '未检测到已登录账号';
          await tester.ensureVisible(button);
          await tester.tap(button);
          await tester.pumpAndSettle();
          expect(find.text('查询失败'), findsOneWidget);
          expect(find.text('未检测到已登录账号'), findsOneWidget);
          expect(find.text('tester@example.com'), findsNothing);
          expect(find.text('1,234,567'), findsNothing);
          if (width >= 820) {
            expect(
              tester.getRect(card).bottom,
              closeTo(
                tester
                    .getRect(find.byKey(const ValueKey('launch-settings-card')))
                    .bottom,
                0.1,
              ),
            );
          }
          expect(tester.takeException(), isNull);
          await tester.pumpWidget(const SizedBox());
        },
      );
    }
  }

  testWidgets(
    'loading prevents duplicate queries and keeps launch buttons available',
    (tester) async {
      SharedPreferences.setMockInitialValues({});
      tester.view.devicePixelRatio = 1;
      tester.view.physicalSize = const Size(1280, 1100);
      addTearDown(tester.view.resetDevicePixelRatio);
      addTearDown(tester.view.resetPhysicalSize);
      final service = PageUsage()..pending = Completer();
      final backend = PageBackend();
      await tester.pumpWidget(
        LauncherApplication(
          backend: backend,
          usageService: service,
          networkFactory: () => NetworkController(preview: true),
        ),
      );
      await tester.pumpAndSettle();
      final button = find.byKey(const ValueKey('query-account-usage'));
      await tester.ensureVisible(button);
      await tester.tap(button);
      await tester.pump();
      expect(find.text('正在查询用量…'), findsOneWidget);
      expect(
        tester.getRect(find.byKey(const ValueKey('account-usage-card'))).bottom,
        closeTo(
          tester
              .getRect(find.byKey(const ValueKey('launch-settings-card')))
              .bottom,
          0.1,
        ),
      );
      expect(tester.widget<FButton>(button).onPress, isNull);
      await tester.tap(find.text('保存设置'));
      await tester.pump();
      expect(backend.calls, contains('save'));
      expect(service.calls, 1);
      service.pending!.complete(AccountUsage.fromJson(usageResponse()));
      await tester.pumpAndSettle();
      expect(find.text('已用 8%'), findsOneWidget);
      expect(tester.takeException(), isNull);
      await tester.pumpWidget(const SizedBox());
    },
  );

  testWidgets('missing token statistics preserve quota display', (
    tester,
  ) async {
    SharedPreferences.setMockInitialValues({});
    tester.view.devicePixelRatio = 1;
    tester.view.physicalSize = const Size(1280, 1100);
    addTearDown(tester.view.resetDevicePixelRatio);
    addTearDown(tester.view.resetPhysicalSize);
    final service = PageUsage();
    service.response['tokenUsage'] = null;
    service.response['tokensError'] = '当前版本不支持 token 统计';
    await tester.pumpWidget(
      LauncherApplication(
        backend: PageBackend(),
        usageService: service,
        networkFactory: () => NetworkController(preview: true),
      ),
    );
    await tester.pumpAndSettle();
    final button = find.byKey(const ValueKey('query-account-usage'));
    await tester.ensureVisible(button);
    await tester.tap(button);
    await tester.pumpAndSettle();
    expect(find.text('已用 8%'), findsOneWidget);
    expect(find.text('暂无数据'), findsNWidgets(2));
    expect(find.text('当前版本不支持 token 统计'), findsOneWidget);
    expect(tester.takeException(), isNull);
    await tester.pumpWidget(const SizedBox());
  });

  final realData = Platform.environment['ACCOUNT_USAGE_RESPONSE'];
  if (realData != null) {
    testWidgets(
      'actual local response renders percentages, timestamps and token statistics',
      (tester) async {
        SharedPreferences.setMockInitialValues({});
        tester.view.devicePixelRatio = 1;
        tester.view.physicalSize = const Size(1280, 1200);
        addTearDown(tester.view.resetDevicePixelRatio);
        addTearDown(tester.view.resetPhysicalSize);
        final service = PageUsage()
          ..response = jsonDecode(
            File(realData).readAsStringSync(),
          ) as Map<String, dynamic>;
        final expected = AccountUsage.fromJson(service.response);
        final boundary = GlobalKey();
        await tester.pumpWidget(
          RepaintBoundary(
            key: boundary,
            child: LauncherApplication(
              backend: PageBackend(),
              usageService: service,
              networkFactory: () => NetworkController(preview: true),
              initialAppearance: 'light',
            ),
          ),
        );
        await tester.pumpAndSettle();
        expect(service.calls, 0);
        final button = find.byKey(const ValueKey('query-account-usage'));
        await tester.ensureVisible(button);
        await tester.tap(button);
        await tester.pumpAndSettle();
        expect(find.text('查询失败'), findsNothing);
        expect(find.text(tokenCount(expected.lifetimeTokens)), findsWidgets);
        expect(find.text(tokenCount(expected.latestDailyTokens)), findsWidgets);
        expect(
          find.byType(FDeterminateProgress),
          findsNWidgets(
            expected.buckets.fold<int>(
              0,
              (n, b) =>
                  n + b.windows.where((w) => w.usedPercent != null).length,
            ),
          ),
        );
        expect(tester.takeException(), isNull);
        await tester.drag(
          find.byType(SingleChildScrollView).first,
          const Offset(0, 2000),
        );
        await tester.pumpAndSettle();
        await screenshot(tester, boundary, 'actual-account-loaded');
        await tester.pumpWidget(const SizedBox());
      },
    );
  }
  if (Platform.environment['LIVE_ACCOUNT_USAGE'] == '1') {
    testWidgets(
      'clicking query reads real local account through native backend',
      (tester) async {
        SharedPreferences.setMockInitialValues({});
        tester.view.devicePixelRatio = 1;
        tester.view.physicalSize = const Size(1280, 1200);
        addTearDown(tester.view.resetDevicePixelRatio);
        addTearDown(tester.view.resetPhysicalSize);
        final service = LiveUsage();
        final backend = PageBackend();
        final discovered = await tester.runAsync(
          () => const NativeBackend().call('discover'),
        );
        backend.detected = discovered!['path'] as String;
        final boundary = GlobalKey();
        await tester.pumpWidget(
          RepaintBoundary(
            key: boundary,
            child: LauncherApplication(
              backend: backend,
              usageService: service,
              networkFactory: () => NetworkController(preview: true),
              initialAppearance: 'light',
            ),
          ),
        );
        await tester.pumpAndSettle();
        expect(service.calls, 0);
        final button = find.byKey(const ValueKey('query-account-usage'));
        await tester.ensureVisible(button);
        await tester.runAsync(() async {
          await tester.tap(button);
          final deadline = DateTime.now().add(const Duration(seconds: 65));
          while (service.result == null && DateTime.now().isBefore(deadline)) {
            await Future<void>.delayed(const Duration(milliseconds: 100));
            if (find.text('查询失败').evaluate().isNotEmpty) break;
          }
        });
        await tester.pumpAndSettle();
        expect(service.calls, 1);
        expect(service.result, isNotNull);
        expect(find.text('查询失败'), findsNothing);
        expect(
          find.text(tokenCount(service.result!.lifetimeTokens)),
          findsOneWidget,
        );
        expect(
          find.text(tokenCount(service.result!.latestDailyTokens)),
          findsOneWidget,
        );
        expect(find.text('5 小时额度'), findsOneWidget);
        expect(find.text('周额度'), findsOneWidget);
        expect(tester.takeException(), isNull);
        await screenshot(tester, boundary, 'live-account-loaded');
        await tester.pumpWidget(const SizedBox());
      },
      timeout: const Timeout(Duration(seconds: 90)),
    );
  }
}
