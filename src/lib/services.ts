export const SERVICE_PORTS: Record<number, string> = {
  20: "ftp", 21: "ftp", 22: "ssh", 23: "telnet", 25: "smtp", 53: "dns",
  67: "dhcp", 68: "dhcp", 80: "http", 110: "pop3", 123: "ntp", 143: "imap",
  161: "snmp", 389: "ldap", 443: "https", 445: "smb", 465: "smtps",
  587: "smtp", 636: "ldaps", 993: "imaps", 995: "pop3s", 1433: "mssql",
  1521: "oracle", 3000: "dev", 3306: "mysql", 3389: "rdp", 5000: "flask",
  5432: "postgres", 5672: "amqp", 5900: "vnc", 6379: "redis", 8000: "http",
  8080: "http-alt", 8443: "https-alt", 8888: "http", 9090: "metrics",
  9200: "elastic", 11211: "memcached", 27017: "mongo",
};

export function svcLabel(port: number): string | undefined {
  return SERVICE_PORTS[port];
}

export function portLabel(port: number): string {
  const svc = SERVICE_PORTS[port];
  return svc ? `${port} · ${svc}` : String(port);
}
