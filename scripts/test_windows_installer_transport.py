#!/usr/bin/env python3
"""Deterministic RC7 Windows installer transport contract tests.

The pure models make edge-case expectations executable. Every test also binds
its expectation to install/v0.2/install.ps1 through a focused static assertion.
No network access or third-party package is used.
"""

from __future__ import annotations

import re
import tempfile
import unittest
from dataclasses import dataclass
from pathlib import Path
from typing import Mapping, Optional, Sequence
from urllib.parse import urljoin, urlsplit, urlunsplit


REPO_ROOT = Path(__file__).resolve().parents[1]
INSTALLER_PATH = REPO_ROOT / "install" / "v0.2" / "install.ps1"
INSTALLER = INSTALLER_PATH.read_text(encoding="utf-8")
TRANSPORT_MATCH = re.search(
    r"(?ms)^function Save-HttpsAsset\s*\{.*?(?=^function |\Z)", INSTALLER
)
TRANSPORT_BODY = TRANSPORT_MATCH.group(0) if TRANSPORT_MATCH else ""
REDIRECT_CODES = {301, 302, 303, 307, 308}
MAX_REDIRECTS = 10


class ContractError(RuntimeError):
    def __init__(self, code: str, message: str):
        super().__init__(message)
        self.code = code


@dataclass(frozen=True)
class ProxyConfig:
    uri: Optional[str]
    source: str
    use_default_credentials: bool


@dataclass(frozen=True)
class Response:
    status: int
    location: Optional[str] = None
    body: bytes = b"asset"
    content_length: Optional[int] = None


@dataclass(frozen=True)
class DownloadResult:
    final_uri: str
    redirects: int
    byte_count: int
    proxy_uri_per_request: tuple[Optional[str], ...]


def canonical_https_uri(value: str, error_code: str) -> str:
    parsed = urlsplit(value)
    if (
        parsed.scheme != "https"
        or not parsed.netloc
        or parsed.username is not None
        or parsed.password is not None
    ):
        raise ContractError(error_code, "unsafe HTTPS URI")
    host = parsed.hostname
    if not host:
        raise ContractError(error_code, "missing host")
    return urlunsplit(
        (
            "https",
            parsed.netloc.lower(),
            parsed.path or "/",
            parsed.query,
            "",
        )
    )


def validate_proxy_uri(value: str) -> str:
    if any(character.isspace() for character in value):
        raise ContractError("PROXY_CONFIGURATION_INVALID", "proxy has whitespace")
    parsed = urlsplit(value)
    if (
        parsed.scheme not in {"http", "https"}
        or not parsed.netloc
        or parsed.hostname is None
        or parsed.username is not None
        or parsed.password is not None
        or parsed.query
        or parsed.fragment
    ):
        raise ContractError("PROXY_CONFIGURATION_INVALID", "invalid proxy URI")
    return urlunsplit(
        (parsed.scheme, parsed.netloc.lower(), parsed.path, "", "")
    )


def resolve_proxy(
    explicit: Optional[str],
    environment: Mapping[str, str],
    system_proxy: Optional[str] = None,
    use_default_credentials: bool = False,
) -> ProxyConfig:
    if explicit:
        return ProxyConfig(
            validate_proxy_uri(explicit), "explicit", use_default_credentials
        )
    for name in ("HTTPS_PROXY", "https_proxy", "HTTP_PROXY", "http_proxy"):
        value = environment.get(name)
        if value:
            return ProxyConfig(
                validate_proxy_uri(value), "env", use_default_credentials
            )
    if system_proxy:
        return ProxyConfig(
            validate_proxy_uri(system_proxy), "system", use_default_credentials
        )
    return ProxyConfig(None, "none", False)


def download_model(
    initial_uri: str,
    responses: Sequence[Response | BaseException],
    proxy: ProxyConfig,
    destination: Path,
) -> DownloadResult:
    current = canonical_https_uri(initial_uri, "UNSAFE_REDIRECT_TARGET")
    visited = {current}
    proxy_history: list[Optional[str]] = []
    redirect_count = 0
    partial = destination.parent / f".{destination.name}.partial-model"

    if destination.exists():
        raise ContractError("ASSET_DOWNLOAD_FAILED", "destination exists")

    try:
        for item in responses:
            proxy_history.append(proxy.uri)
            if isinstance(item, BaseException):
                raise item
            if item.status in REDIRECT_CODES:
                if not item.location:
                    raise ContractError(
                        "HTTP_REDIRECT_MISSING_LOCATION", "missing Location"
                    )
                if redirect_count >= MAX_REDIRECTS:
                    raise ContractError(
                        "HTTP_REDIRECT_LIMIT", "redirect limit exceeded"
                    )
                next_uri = canonical_https_uri(
                    urljoin(current, item.location), "UNSAFE_REDIRECT_TARGET"
                )
                if next_uri in visited:
                    raise ContractError("HTTP_REDIRECT_LOOP", "redirect loop")
                visited.add(next_uri)
                current = next_uri
                redirect_count += 1
                continue
            if item.status != 200:
                raise ContractError(
                    "HTTP_STATUS_REJECTED", f"HTTP status {item.status}"
                )
            if not item.body:
                raise ContractError("ASSET_DOWNLOAD_FAILED", "empty body")
            partial.write_bytes(item.body)
            actual = partial.stat().st_size
            if item.content_length is not None and item.content_length != actual:
                raise ContractError(
                    "ASSET_LENGTH_MISMATCH", "Content-Length mismatch"
                )
            if destination.exists():
                raise ContractError(
                    "ASSET_DOWNLOAD_FAILED", "destination appeared"
                )
            partial.rename(destination)
            return DownloadResult(
                current, redirect_count, actual, tuple(proxy_history)
            )
        raise ContractError("HTTP_REQUEST_FAILED", "response sequence exhausted")
    finally:
        if partial.exists():
            partial.unlink()


class InstallerContractCase(unittest.TestCase):
    maxDiff = None

    def require(self, *patterns: str) -> None:
        missing = [
            pattern
            for pattern in patterns
            if re.search(pattern, INSTALLER, re.MULTILINE | re.IGNORECASE) is None
        ]
        if missing:
            self.fail(
                "RC7 installer contract missing in "
                f"{INSTALLER_PATH.relative_to(REPO_ROOT)}: "
                + ", ".join(missing)
            )

    def require_proxy_transport(self) -> None:
        self.require(
            r"\[Uri\]\s*\$ProxyUri",
            r"\[switch\]\s*\$ProxyUseDefaultCredentials",
            r"HttpClientHandler",
            r"ResponseHeadersRead",
        )

    def require_in_transport(self, *patterns: str) -> None:
        if not TRANSPORT_BODY:
            self.fail("Save-HttpsAsset is missing from install/v0.2/install.ps1")
        missing = [
            pattern
            for pattern in patterns
            if re.search(
                pattern, TRANSPORT_BODY, re.MULTILINE | re.IGNORECASE
            )
            is None
        ]
        if missing:
            self.fail(
                "RC7 Save-HttpsAsset contract missing: " + ", ".join(missing)
            )

    def assert_contract_error(self, code: str, callback) -> ContractError:
        with self.assertRaises(ContractError) as captured:
            callback()
        self.assertEqual(captured.exception.code, code)
        return captured.exception

    def model_download(
        self,
        responses: Sequence[Response | BaseException],
        proxy: Optional[ProxyConfig] = None,
    ) -> DownloadResult:
        root = tempfile.TemporaryDirectory()
        self.addCleanup(root.cleanup)
        destination = Path(root.name) / "asset.bin"
        return download_model(
            "https://example.invalid/releases/asset.bin",
            responses,
            proxy or ProxyConfig(None, "none", False),
            destination,
        )

    def test_01_direct_https_no_proxy(self) -> None:
        config = resolve_proxy(None, {})
        self.assertEqual(config, ProxyConfig(None, "none", False))
        self.require_proxy_transport()
        self.require(r"UseProxy\s*=\s*\$false")

    def test_02_explicit_proxy_uri(self) -> None:
        config = resolve_proxy("http://127.0.0.1:8080", {})
        self.assertEqual(config.source, "explicit")
        self.require_proxy_transport()
        self.require(r"WebProxy")

    def test_03_https_proxy_resolution(self) -> None:
        config = resolve_proxy(
            None, {"HTTPS_PROXY": "http://127.0.0.1:8081"}
        )
        self.assertEqual(config.uri, "http://127.0.0.1:8081")
        self.require(r"HTTPS_PROXY")

    def test_04_http_proxy_fallback(self) -> None:
        config = resolve_proxy(
            None, {"HTTP_PROXY": "http://127.0.0.1:8082"}
        )
        self.assertEqual(config.source, "env")
        self.require(r"HTTP_PROXY")

    def test_05_lowercase_environment_variants(self) -> None:
        https = resolve_proxy(
            None, {"https_proxy": "http://127.0.0.1:8083"}
        )
        http = resolve_proxy(None, {"http_proxy": "http://127.0.0.1:8084"})
        self.assertNotEqual(https.uri, http.uri)
        self.require(r"https_proxy", r"http_proxy")

    def test_06_explicit_proxy_wins(self) -> None:
        config = resolve_proxy(
            "http://127.0.0.1:8090",
            {"HTTPS_PROXY": "http://127.0.0.1:8091"},
        )
        self.assertEqual(config.uri, "http://127.0.0.1:8090")
        self.require_proxy_transport()
        self.require(r"ProxySource")

    def test_07_no_proxy_remains_valid(self) -> None:
        result = self.model_download([Response(200)])
        self.assertEqual(result.byte_count, 5)
        self.require_proxy_transport()
        self.require(r"DefaultWebProxy")

    def test_08_invalid_proxy_scheme_rejected(self) -> None:
        self.assert_contract_error(
            "PROXY_CONFIGURATION_INVALID",
            lambda: resolve_proxy("socks5://127.0.0.1:1080", {}),
        )
        self.require(r"PROXY_CONFIGURATION_INVALID")

    def test_09_proxy_userinfo_rejected(self) -> None:
        self.assert_contract_error(
            "PROXY_CONFIGURATION_INVALID",
            lambda: resolve_proxy("http://user:pass@127.0.0.1:8080", {}),
        )
        self.require(r"PROXY_CONFIGURATION_INVALID", r"UserInfo")

    def test_10_default_credentials_are_opt_in(self) -> None:
        off = resolve_proxy("http://127.0.0.1:8080", {})
        on = resolve_proxy(
            "http://127.0.0.1:8080", {}, use_default_credentials=True
        )
        self.assertFalse(off.use_default_credentials)
        self.assertTrue(on.use_default_credentials)
        self.require(
            r"ProxyUseDefaultCredentials",
            r"DefaultNetworkCredentials",
        )

    def test_11_absolute_302_redirect(self) -> None:
        result = self.model_download(
            [
                Response(302, "https://example.invalid/assets/asset.bin"),
                Response(200),
            ]
        )
        self.assertEqual(result.redirects, 1)
        self.require(r"HttpStatusCode\]::Found|301.*302.*303.*307.*308")

    def test_12_relative_302_redirect(self) -> None:
        result = self.model_download(
            [Response(302, "../assets/asset.bin"), Response(200)]
        )
        self.assertEqual(
            result.final_uri, "https://example.invalid/assets/asset.bin"
        )
        self.require(r"new-object\s+System\.Uri|Uri\]\s*::new")

    def test_13_303_redirect(self) -> None:
        self.assertEqual(
            self.model_download(
                [Response(303, "/asset.bin"), Response(200)]
            ).redirects,
            1,
        )
        self.require(r"303|SeeOther")

    def test_14_307_redirect(self) -> None:
        self.assertEqual(
            self.model_download(
                [Response(307, "/asset.bin"), Response(200)]
            ).redirects,
            1,
        )
        self.require(r"307|TemporaryRedirect")

    def test_15_308_redirect(self) -> None:
        self.assertEqual(
            self.model_download(
                [Response(308, "/asset.bin"), Response(200)]
            ).redirects,
            1,
        )
        self.require(r"308|PermanentRedirect")

    def test_16_missing_location_rejected(self) -> None:
        self.assert_contract_error(
            "HTTP_REDIRECT_MISSING_LOCATION",
            lambda: self.model_download([Response(302)]),
        )
        self.require(r"HTTP_REDIRECT_MISSING_LOCATION")

    def test_17_http_downgrade_rejected(self) -> None:
        self.assert_contract_error(
            "UNSAFE_REDIRECT_TARGET",
            lambda: self.model_download(
                [Response(302, "http://example.invalid/asset.bin")]
            ),
        )
        self.require(r"UNSAFE_REDIRECT_TARGET")

    def test_18_redirect_userinfo_rejected(self) -> None:
        self.assert_contract_error(
            "UNSAFE_REDIRECT_TARGET",
            lambda: self.model_download(
                [Response(302, "https://user@example.invalid/asset.bin")]
            ),
        )
        self.require(r"UNSAFE_REDIRECT_TARGET", r"UserInfo")

    def test_19_redirect_loop_rejected(self) -> None:
        self.assert_contract_error(
            "HTTP_REDIRECT_LOOP",
            lambda: self.model_download(
                [Response(302, "/second"), Response(302, "/second")]
            ),
        )
        self.require(r"HTTP_REDIRECT_LOOP")

    def test_20_redirect_limit_enforced(self) -> None:
        redirects = [
            Response(302, f"/hop-{index}") for index in range(MAX_REDIRECTS + 1)
        ]
        self.assert_contract_error(
            "HTTP_REDIRECT_LIMIT", lambda: self.model_download(redirects)
        )
        self.require(r"HTTP_REDIRECT_LIMIT", r"\b10\b")

    def test_21_final_non_200_rejected(self) -> None:
        self.assert_contract_error(
            "HTTP_STATUS_REJECTED",
            lambda: self.model_download([Response(206)]),
        )
        self.require(r"HTTP_STATUS_REJECTED")

    def test_22_empty_body_rejected(self) -> None:
        self.assert_contract_error(
            "ASSET_DOWNLOAD_FAILED",
            lambda: self.model_download([Response(200, body=b"")]),
        )
        self.require(r"ASSET_DOWNLOAD_FAILED")

    def test_23_content_length_mismatch_rejected(self) -> None:
        self.assert_contract_error(
            "ASSET_LENGTH_MISMATCH",
            lambda: self.model_download(
                [Response(200, body=b"asset", content_length=6)]
            ),
        )
        self.require(r"ASSET_LENGTH_MISMATCH", r"ContentLength|Content-Length")

    def test_24_existing_destination_preserved(self) -> None:
        with tempfile.TemporaryDirectory() as root:
            destination = Path(root) / "asset.bin"
            destination.write_bytes(b"existing")
            self.assert_contract_error(
                "ASSET_DOWNLOAD_FAILED",
                lambda: download_model(
                    "https://example.invalid/asset.bin",
                    [Response(200)],
                    ProxyConfig(None, "none", False),
                    destination,
                ),
            )
            self.assertEqual(destination.read_bytes(), b"existing")
        self.require_in_transport(
            r"FileMode\]::CreateNew",
            r"partial",
            r"\[IO\.File\]::Move|File\]::Move",
        )
        self.assertNotRegex(
            TRANSPORT_BODY,
            r"-OutFile\s+\$Destination",
            "transport writes directly to the final destination",
        )

    def test_25_partial_file_cleaned_after_failure(self) -> None:
        with tempfile.TemporaryDirectory() as root:
            destination = Path(root) / "asset.bin"
            self.assert_contract_error(
                "ASSET_LENGTH_MISMATCH",
                lambda: download_model(
                    "https://example.invalid/asset.bin",
                    [Response(200, body=b"x", content_length=2)],
                    ProxyConfig(None, "none", False),
                    destination,
                ),
            )
            self.assertEqual(list(Path(root).iterdir()), [])
        self.require_in_transport(
            r"partial",
            r"finally",
            r"Remove-Item\s+-LiteralPath\s+\$[A-Za-z]*partial",
        )

    def test_26_original_exception_type_and_message_preserved(self) -> None:
        original = RuntimeError("synthetic transport failure")
        with self.assertRaisesRegex(RuntimeError, "synthetic transport failure"):
            self.model_download([original])
        self.require(
            r"HTTP_REQUEST_FAILED",
            r"GetType\(\)\.FullName|ExceptionType",
            r"function\s+Get-OriginalTransportException",
        )
        self.assertGreaterEqual(
            len(
                re.findall(
                    r"Get-OriginalTransportException\s+`?\s*-Exception",
                    INSTALLER,
                    re.IGNORECASE,
                )
            ),
            2,
            "request and streamed-body failures must unwrap invocation wrappers",
        )

    def test_27_exception_without_response_property(self) -> None:
        class NoResponseError(RuntimeError):
            pass

        with self.assertRaisesRegex(NoResponseError, "no response property"):
            self.model_download([NoResponseError("no response property")])
        self.require(r"HttpClientHandler")
        response_reads = re.findall(
            r"\$_\.Exception\.Response", INSTALLER, re.IGNORECASE
        )
        if response_reads:
            self.require(r"PSObject\.Properties\[['\"]Response['\"]\]")

    def test_28_no_unsafe_direct_exception_response_access(self) -> None:
        response_reads = re.findall(
            r"\$_\.Exception\.Response", INSTALLER, re.IGNORECASE
        )
        if response_reads:
            self.require(r"PSObject\.Properties\[['\"]Response['\"]\]")

    def test_29_no_production_maximum_redirection_zero(self) -> None:
        self.assertNotRegex(
            INSTALLER,
            r"MaximumRedirection\s+0",
            "production MaximumRedirection 0 remains in install.ps1",
        )
        self.require(r"AllowAutoRedirect\s*=\s*\$false")

    def test_30_same_proxy_retained_across_redirects(self) -> None:
        proxy = resolve_proxy("http://127.0.0.1:8080", {})
        result = self.model_download(
            [
                Response(302, "/one"),
                Response(307, "/two"),
                Response(200),
            ],
            proxy,
        )
        self.assertEqual(
            result.proxy_uri_per_request,
            ("http://127.0.0.1:8080",) * 3,
        )
        self.require_proxy_transport()
        self.require(r"AllowAutoRedirect\s*=\s*\$false")


if __name__ == "__main__":
    unittest.main(verbosity=2)
