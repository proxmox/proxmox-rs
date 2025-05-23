From root@host.example.com  Fri May 23 11:59:32 2025
Return-Path: <root@host.example.com>
X-Original-To: root
Delivered-To: root@host.example.com
Received: by host.example.com (Postfix, from userid 0)
	id 25CE920081A; Fri, 23 May 2025 11:59:32 +0200 (CEST)
Content-Type: multipart/alternative;
	boundary="----_=_NextPart_002_1747994372"
MIME-Version: 1.0
Subject: vzdump backup status (host.example.com): backup successful 
From: Proxmox VE <root@host.example.com>
To: root@host.example.com
Date: Fri, 23 May 2025 11:59:32 +0200
Auto-Submitted: auto-generated;
Message-Id: <20250523095932.25CE920081A@host.example.com>

This is a multi-part message in MIME format.

------_=_NextPart_002_1747994372
Content-Type: text/plain;
	charset="UTF-8"
Content-Transfer-Encoding: 8bit


Details
=======
VMID    Name         Status    Time    Size           Filename                                                       
187     smtp-sink    ok        11s     430.758 MiB    /var/lib/vz/dump/vzdump-lxc-187-2025_05_23-11_59_21.tar.zst    

Total running time: 11s
Total size: 430.758 MiB

Logs
====
vzdump 187 --compress zstd --notes-template '{{guestname}}' --remove 0 --storage local --mode snapshot --notification-mode notification-system --node host


187: 2025-05-23 11:59:21 INFO: Starting Backup of VM 187 (lxc)
187: 2025-05-23 11:59:21 INFO: status = stopped
187: 2025-05-23 11:59:21 INFO: backup mode: stop
187: 2025-05-23 11:59:21 INFO: ionice priority: 7
187: 2025-05-23 11:59:21 INFO: CT Name: smtp-sink
187: 2025-05-23 11:59:21 INFO: including mount point rootfs ('/') in backup
187: 2025-05-23 11:59:21 INFO: creating vzdump archive '/var/lib/vz/dump/vzdump-lxc-187-2025_05_23-11_59_21.tar.zst'
187: 2025-05-23 11:59:32 INFO: Total bytes written: 1519319040 (1.5GiB, 135MiB/s)
187: 2025-05-23 11:59:32 INFO: archive file size: 430MB
187: 2025-05-23 11:59:32 INFO: adding notes to backup
187: 2025-05-23 11:59:32 INFO: Finished Backup of VM 187 (00:00:11)



------_=_NextPart_002_1747994372
Content-Type: text/html;
	charset="UTF-8"
Content-Transfer-Encoding: 8bit

<html>
    <body>
        
        <h1 style="font-size: 1.2em">Details</h1>
        <table style="border: 1px solid;border-collapse=collapse;">
            <tr>
                <th style="border: 1px solid;border-collapse=collapse;">VMID</th>
                <th style="border: 1px solid;border-collapse=collapse;">Name</th>
                <th style="border: 1px solid;border-collapse=collapse;">Status</th>
                <th style="border: 1px solid;border-collapse=collapse;">Time</th>
                <th style="border: 1px solid;border-collapse=collapse;">Size</th>
                <th style="border: 1px solid;border-collapse=collapse;">Filename</th>
            </tr>
        
            <tr>
                <td style="border: 1px solid;border-collapse=collapse;">187</th>
                <td style="border: 1px solid;border-collapse=collapse;">smtp-sink</th>
                <td style="border: 1px solid;border-collapse=collapse;">ok</th>
                <td style="border: 1px solid;border-collapse=collapse;">11s</th>
                <td style="border: 1px solid;border-collapse=collapse;">430.758 MiB</th>
                <td style="border: 1px solid;border-collapse=collapse;">/var/lib/vz/dump/vzdump-lxc-187-2025_05_23-11_59_21.tar.zst</th>
            </tr>
        
        </table>
        <br/>
        Total running time: 11s<br/>
        Total size: 430.758 MiB<br/>
        <h1 style="font-size: 1.2em">Logs</h1>
        <pre>vzdump 187 --compress zstd --notes-template &#x27;{{guestname}}&#x27; --remove 0 --storage local --mode snapshot --notification-mode notification-system --node host


187: 2025-05-23 11:59:21 INFO: Starting Backup of VM 187 (lxc)
187: 2025-05-23 11:59:21 INFO: status &#x3D; stopped
187: 2025-05-23 11:59:21 INFO: backup mode: stop
187: 2025-05-23 11:59:21 INFO: ionice priority: 7
187: 2025-05-23 11:59:21 INFO: CT Name: smtp-sink
187: 2025-05-23 11:59:21 INFO: including mount point rootfs (&#x27;/&#x27;) in backup
187: 2025-05-23 11:59:21 INFO: creating vzdump archive &#x27;/var/lib/vz/dump/vzdump-lxc-187-2025_05_23-11_59_21.tar.zst&#x27;
187: 2025-05-23 11:59:32 INFO: Total bytes written: 1519319040 (1.5GiB, 135MiB/s)
187: 2025-05-23 11:59:32 INFO: archive file size: 430MB
187: 2025-05-23 11:59:32 INFO: adding notes to backup
187: 2025-05-23 11:59:32 INFO: Finished Backup of VM 187 (00:00:11)

</pre>
    </body>
</html>

------_=_NextPart_002_1747994372--
