# IRC RFC Reference Documents

This directory contains the core IETF RFCs that define the IRC protocol.

## Documents

| RFC | Title | Size | Description |
|-----|-------|------|-------------|
| [RFC 2810](rfc2810.txt) | IRC: Architecture | 19 KB | Network topology, server/client model, services |
| [RFC 2811](rfc2811.txt) | IRC: Channel Management | 41 KB | Channel types, modes, operators, membership |
| [RFC 2812](rfc2812.txt) | IRC: Client Protocol | 123 KB | Message format, commands, numeric replies |
| [RFC 2813](rfc2813.txt) | IRC: Server Protocol | 57 KB | Server-to-server communication, state sync |

## Reading Order

1. **RFC 2810** - Start here to understand the overall architecture
2. **RFC 2812** - The bulk of implementation details for client-server
3. **RFC 2811** - Channel specifics (referenced by 2812)
4. **RFC 2813** - Only needed for server-to-server linking

## Key Sections by Topic

### Message Format (RFC 2812 Section 2)
- BNF grammar for messages
- 512-byte limit
- Prefix, command, parameters structure

### Connection Registration (RFC 2812 Section 3.1)
- PASS, NICK, USER, OPER, QUIT commands
- Registration sequence

### Channel Operations (RFC 2812 Section 3.2)
- JOIN, PART, MODE, TOPIC, NAMES, LIST, INVITE, KICK

### Messaging (RFC 2812 Section 3.3)
- PRIVMSG, NOTICE

### Server Queries (RFC 2812 Section 3.4)
- MOTD, LUSERS, VERSION, STATS, LINKS, TIME, etc.

### User Queries (RFC 2812 Section 3.6)
- WHO, WHOIS, WHOWAS

### Numeric Replies (RFC 2812 Section 5)
- Complete list of 001-502 reply codes

### Channel Modes (RFC 2811 Section 4)
- Member modes: O, o, v
- Channel flags: a, i, m, n, q, p, s, r, t, k, l
- Access masks: b, e, I

## Related Documents (Not Included)

- **RFC 1459** - Original IRC specification (superseded by 2810-2813)
- **IRCv3 Specifications** - Modern extensions at https://ircv3.net/
  - Capability negotiation
  - SASL authentication
  - Message tags
  - Server-time
  - Account tracking
