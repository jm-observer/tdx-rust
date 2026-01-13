# 通达信协议测试数据

本目录包含通达信协议各个接口的测试数据，用于 Rust 版本的单元测试和集成测试。

## 文件结构

```
tdx-test/
├── README.md              # 本文件
├── index.json             # 测试文件索引
├── test_data.rs           # Rust 测试数据结构定义
├── example_usage.rs       # 使用示例代码
├── connect.json           # 建立连接测试数据
├── heartbeat.json         # 心跳包测试数据
├── count.json             # 获取股票数量测试数据
├── code.json              # 获取股票代码列表测试数据
├── quote.json             # 行情信息（5档报价）测试数据
├── kline.json             # K线数据测试数据
├── minute.json            # 分时数据测试数据
├── trade.json             # 分时成交测试数据
├── history_minute.json     # 历史分时数据测试数据
├── history_trade.json     # 历史分时成交测试数据
├── call_auction.json       # 集合竞价测试数据
└── gbbq.json              # 除权除息测试数据
```

## JSON 文件格式

每个接口对应一个 JSON 文件，包含以下字段：

- `name`: 接口名称（中文）
- `type`: 类型名称（如 "TypeConnect"）
- `type_value`: 类型值（十六进制，如 "0x000D"）
- `description`: 接口说明
- `request`: 请求帧的十六进制字符串（完整帧）
- `request_description`: 请求帧格式说明
- `request_data`: 请求数据域（可选，不含帧头）
- `response`: 响应帧的十六进制字符串（完整帧，可能包含压缩数据）
- `response_description`: 响应帧格式说明
- `response_data`: 响应数据域（可选，解压后，不含帧头）
- `params`: 请求参数说明（JSON 对象）
- `notes`: 其他说明（可选）

## 数据格式说明

### 请求帧格式

```
+--------+--------+--------+--------+--------+--------+--------+
| Prefix | MsgID  | Control| Length | Length | Type   | Data   |
| (1字节) | (4字节) | (1字节) | (2字节) | (2字节) | (2字节) | (变长) |
+--------+--------+--------+--------+--------+--------+--------+
```

- Prefix: `0x0C`（固定）
- MsgID: 消息ID，小端序，自增
- Control: 控制码，通常为 `0x01`
- Length: 数据长度（包含Type字段的2字节），小端序，重复两次
- Type: 请求类型，小端序
- Data: 数据域，变长

### 响应帧格式

```
+--------+--------+--------+--------+--------+--------+--------+--------+
| Prefix | Control| MsgID  |Unknown | Type   |ZipLen  | Length | Data   |
| (4字节) | (1字节) | (4字节) | (1字节) | (2字节) | (2字节) | (2字节) | (变长) |
+--------+--------+--------+--------+--------+--------+--------+--------+
```

- Prefix: `0xB1CB7400`（小端序，固定）
- Control: 控制码，`0x1C`表示成功，`0x0C`表示错误
- MsgID: 消息ID，对应请求的MsgID，小端序
- Unknown: 未知字段，通常为 `0x00`
- Type: 响应类型，对应请求的Type，小端序
- ZipLength: 压缩数据长度，小端序
- Length: 未压缩数据长度，小端序
- Data: 数据域，如果 ZipLength != Length，则使用 zlib 压缩

## 使用示例

### 方法1：使用 test_data.rs 结构

```rust
use test_data::TestData;

// 加载测试数据
let test_data = TestData::load("connect")?;

// 解码请求帧
let request_bytes = test_data.decode_request()?;

// 解码响应帧
let response_bytes = test_data.decode_response()?;

// 解析帧头
assert_eq!(request_bytes[0], 0x0C);
let msg_id = u32::from_le_bytes([
    request_bytes[1], request_bytes[2], 
    request_bytes[3], request_bytes[4]
]);
```

### 方法2：直接读取 JSON 文件

```rust
use serde_json;
use hex;

// 读取测试数据
let content = std::fs::read_to_string("tdx-test/connect.json")?;
let data: serde_json::Value = serde_json::from_str(&content)?;

// 解码请求帧
let request_hex = data["request"].as_str().unwrap();
let request_bytes = hex::decode(request_hex)?;

// 解析帧头
assert_eq!(request_bytes[0], 0x0C);
```

### 方法3：使用 include_str! 宏（编译时嵌入）

```rust
const CONNECT_TEST_DATA: &str = include_str!("tdx-test/connect.json");

#[test]
fn test_connect() {
    let data: serde_json::Value = serde_json::from_str(CONNECT_TEST_DATA).unwrap();
    let request_hex = data["request"].as_str().unwrap();
    let request_bytes = hex::decode(request_hex).unwrap();
    // ... 测试逻辑
}
```

## 测试接口列表

| 文件名 | 接口名称 | 类型值 | 说明 |
|--------|----------|--------|------|
| connect.json | 建立连接 | 0x000D | 客户端连接服务器后首先发送 |
| heartbeat.json | 心跳包 | 0x0004 | 每30秒发送一次 |
| count.json | 获取股票数量 | 0x044E | 获取指定交易所的股票数量 |
| code.json | 获取股票代码列表 | 0x0450 | 一次返回1000只股票 |
| quote.json | 行情信息 | 0x053E | 5档买卖盘报价 |
| kline.json | K线数据 | 0x052D | 支持多种周期 |
| minute.json | 分时数据 | 0x051D | 当天分时数据 |
| trade.json | 分时成交 | 0x0FC5 | 当天分时成交明细 |
| history_minute.json | 历史分时数据 | 0x0FB4 | 指定日期的分时数据 |
| history_trade.json | 历史分时成交 | 0x0FB5 | 指定日期的分时成交 |
| call_auction.json | 集合竞价 | 0x056A | 当天集合竞价数据 |
| gbbq.json | 除权除息 | 0x000F | 除权除息信息 |

## 验证测试数据

### 使用 VSCode 任务（推荐）

项目已配置 VSCode 任务，可以直接在 VSCode 中运行验证测试：

1. **打开命令面板**：
   - Windows/Linux: `Ctrl+Shift+P`
   - Mac: `Cmd+Shift+P`

2. **选择任务**：
   - 输入 "Tasks: Run Task"
   - 选择以下任务之一：
     - **tdx-test: 验证所有测试数据** - 运行所有验证测试（默认测试任务）
     - **tdx-test: 验证连接测试数据** - 只验证连接接口
     - **tdx-test: 验证心跳包测试数据** - 只验证心跳包接口
     - **tdx-test: 验证股票数量测试数据** - 只验证股票数量接口
     - **tdx-test: 验证行情信息测试数据** - 只验证行情信息接口
     - **tdx-test: 验证K线测试数据** - 只验证K线接口
     - **tdx-test: 初始化 Go 模块** - 初始化/更新 Go 模块依赖

3. **快捷键**：
   - 运行默认测试任务：`Ctrl+Shift+T` (Windows/Linux) 或 `Cmd+Shift+T` (Mac)

### 使用命令行

```bash
# 进入测试目录
cd tdx-test

# 初始化 Go 模块（首次运行）
go mod tidy

# 运行所有验证测试
go test -v -run TestVerifyAll

# 运行特定测试
go test -v -run TestVerifyConnect
go test -v -run TestVerifyCount
go test -v -run TestVerifyQuote
```

## 注意事项

1. **字节序**: 所有多字节数值均为小端序（Little-Endian）
2. **压缩**: 响应数据可能使用 zlib 压缩，需要检查 ZipLength 和 Length
3. **编码**: 股票名称使用 GBK/GB18030 编码，需要转换为 UTF-8
4. **变长编码**: 价格和数量使用变长整数编码，需要正确解析
5. **测试数据**: 部分测试数据可能不完整，实际使用时需要根据协议文档补充

## 参考文档

- [tdx-protocol.md](../tdx-protocol.md) - 完整的协议文档
- [injoyai/tdx](https://github.com/injoyai/tdx) - Go 语言参考实现
- [VERIFICATION_RESULTS.md](./VERIFICATION_RESULTS.md) - 验证结果报告
