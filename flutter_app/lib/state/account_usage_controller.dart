import 'package:flutter/foundation.dart';

import '../services/account_usage.dart';

class AccountUsageController extends ChangeNotifier {
  AccountUsageController(this.service);
  final AccountUsageService service;
  AccountUsage? data;
  String? error;
  bool busy = false;
  bool useLocalTime = true;
  bool _disposed = false;

  void setLocalTime(bool value) {
    if (_disposed || useLocalTime == value) return;
    useLocalTime = value;
    notifyListeners();
  }

  // There is deliberately no initialization query, timer, or persistent cache.
  Future<void> query(String desktopPath) async {
    if (busy || _disposed) return;
    busy = true;
    data = null;
    error = null;
    notifyListeners();
    try {
      final result = await service.query(desktopPath);
      if (!_disposed) data = result;
    } catch (e) {
      if (!_disposed) {
        error = e.toString().replaceFirst(RegExp(r'^Bad state: '), '');
      }
    } finally {
      if (!_disposed) {
        busy = false;
        notifyListeners();
      }
    }
  }

  @override
  void dispose() {
    _disposed = true;
    super.dispose();
  }
}
