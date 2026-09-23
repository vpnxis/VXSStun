//! Read-only TUN readiness checks. Xray owns adapter, addresses, DNS and routes.
//! Never touch another VPN's interfaces or global proxy/firewall settings.
use anyhow::{ensure, Context, Result};
use serde::Deserialize;
use serde_json::json;

// Pick a live physical default route, not an arbitrary Up adapter or another VPN.
pub fn physical_interface() -> Result<String> {
    let value = crate::windows::powershell(r#"
$ErrorActionPreference='Stop'
[Console]::OutputEncoding=[System.Text.UTF8Encoding]::new()
$physical=@(Get-NetAdapter -Physical | Where-Object Status -eq 'Up' | ForEach-Object ifIndex)
$routes=@(Get-NetRoute -PolicyStore ActiveStore | Where-Object { $_.DestinationPrefix -in @('0.0.0.0/0','::/0') -and $_.InterfaceIndex -in $physical } | Sort-Object @{Expression={$_.RouteMetric+$_.InterfaceMetric}})
if($routes.Count -eq 0){throw 'No physical default route'}
@{name=$routes[0].InterfaceAlias} | ConvertTo-Json -Compress
"#, &serde_json::Value::Null).context("Не найдено активное физическое подключение Windows. Проверьте Wi-Fi/Ethernet")?;
    let v: serde_json::Value = serde_json::from_str(&value)?;
    let name = v["name"].as_str().context("Нет сетевого интерфейса")?;
    ensure!(
        !name.is_empty() && name.len() <= 256,
        "Некорректное имя интерфейса"
    );
    Ok(name.into())
}

#[derive(Deserialize)]
struct Readiness {
    up: bool,
    ipv4: bool,
    ipv6: bool,
    route4: bool,
    route6: bool,
    dns: bool,
}
fn validate(raw: &str) -> Result<()> {
    let s: Readiness = serde_json::from_str(raw).context("Windows не вернула состояние TUN")?;
    ensure!(s.up, "TUN-интерфейс не поднят");
    ensure!(s.ipv4 && s.ipv6, "TUN не получил IPv4/IPv6-адреса");
    ensure!(
        s.route4 && s.route6,
        "Маршруты VPN не установлены: соединение не подтверждено"
    );
    ensure!(s.dns, "DNS на VPN-интерфейсе не настроен");
    Ok(())
}
pub fn verify(name: &str) -> Result<()> {
    let raw = crate::windows::powershell(
        r#"
$ErrorActionPreference='Stop'
$p=$env:VXSSTUN_TASK_JSON | ConvertFrom-Json
$a=Get-NetAdapter -Name $p.name -ErrorAction Stop
$ips=@(Get-NetIPAddress -InterfaceIndex $a.ifIndex -ErrorAction Stop)
$routes=@(Get-NetRoute -InterfaceIndex $a.ifIndex -PolicyStore ActiveStore -ErrorAction Stop)
$dns=@(Get-DnsClientServerAddress -InterfaceIndex $a.ifIndex | ForEach-Object ServerAddresses)
@{up=($a.Status -eq 'Up');ipv4=(@($ips | Where-Object IPAddress -eq '172.29.254.1').Count -gt 0);ipv6=(@($ips | Where-Object IPAddress -eq 'fd58:7665:6e6f::1').Count -gt 0);route4=(@($routes | Where-Object DestinationPrefix -eq '0.0.0.0/0').Count -gt 0);route6=(@($routes | Where-Object DestinationPrefix -eq '::/0').Count -gt 0);dns=('1.1.1.1' -in $dns)} | ConvertTo-Json -Compress
"#,
        &json!({"name":name}),
    )?;
    validate(&raw)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn proxy_health_cannot_substitute_for_tun_readiness() {
        let mut state =
            json!({"up":true,"ipv4":true,"ipv6":true,"route4":true,"route6":true,"dns":true});
        assert!(validate(&state.to_string()).is_ok());
        for field in ["up", "ipv4", "ipv6", "route4", "route6", "dns"] {
            state[field] = json!(false);
            assert!(validate(&state.to_string()).is_err(), "{field}");
            state[field] = json!(true);
        }
        assert!(validate("{}").is_err());
        assert!(validate("null").is_err());
    }
}
