"""Tests for startup security warnings."""



from deepseek_web_api.core import server_security


class TestServerSecurity:
    def test_is_loopback_host(self):
        assert server_security.is_loopback_host("127.0.0.1") is True
        assert server_security.is_loopback_host("localhost") is True
        assert server_security.is_loopback_host("[::1]") is True
        assert server_security.is_loopback_host("0.0.0.0") is False

    def test_collect_startup_security_warnings_for_open_defaults(self, monkeypatch):
        monkeypatch.setattr(server_security, "get_server_host", lambda: "127.0.0.1")
        monkeypatch.setattr(server_security, "get_local_api_key", lambda: "")
        monkeypatch.setattr(server_security, "get_cors_origins", lambda: ["*"])

        warnings = server_security.collect_startup_security_warnings()

        assert any("Local API auth is disabled" in warning for warning in warnings)
        assert any("CORS allows all origins" in warning for warning in warnings)
        assert not any("not loopback" in warning for warning in warnings)

    def test_collect_startup_security_warnings_for_remote_exposure(self, monkeypatch):
        monkeypatch.setattr(server_security, "get_server_host", lambda: "0.0.0.0")
        monkeypatch.setattr(server_security, "get_local_api_key", lambda: "")
        monkeypatch.setattr(server_security, "get_cors_origins", lambda: ["https://app.example.com"])

        warnings = server_security.collect_startup_security_warnings()

        assert any("not loopback" in warning for warning in warnings)
        assert any("unsafe" in warning for warning in warnings)

    def test_collect_startup_security_warnings_for_hardened_config(self, monkeypatch):
        monkeypatch.setattr(server_security, "get_server_host", lambda: "127.0.0.1")
        monkeypatch.setattr(server_security, "get_local_api_key", lambda: "secret-token")
        monkeypatch.setattr(server_security, "get_cors_origins", lambda: ["https://app.example.com"])

        warnings = server_security.collect_startup_security_warnings()

        assert warnings == []
