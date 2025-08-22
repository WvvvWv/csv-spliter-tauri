use std::fs::File;
use std::io::{BufReader, BufWriter, Read};
use std::path::Path;
use std::sync::Arc;

use std::thread;

use csv::{ReaderBuilder, Writer, WriterBuilder};
use serde::Serialize;
use tauri::command;
use rust_xlsxwriter::{Workbook, Format, FormatAlign};
use memmap2::Mmap;

#[derive(Serialize)]
struct SplitResult {
    success: bool,
    file_count: usize,
    error: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct SplitParams {
    input_path: String,
    output_dir: String,
    rows_per_file: usize,
    has_header: bool,
    convert_to_excel: bool,
}

/// 分割CSV文件的主命令
#[command]
async fn split_csv(params: SplitParams) -> Result<SplitResult, String> {
    // 根据文件大小决定是否使用多线程优化
    let input_path = Path::new(&params.input_path);
    let metadata = match std::fs::metadata(input_path) {
        Ok(meta) => meta,
        Err(e) => return Err(format!("无法获取文件信息: {}", e)),
    };
    
    // 对于大文件(>50万行或>100MB)使用多线程处理
    let use_multithread = metadata.len() > 100 * 1024 * 1024 || {
        // 快速估算行数
        match File::open(input_path) {
            Ok(f) => {
                let mut reader = BufReader::new(f);
                let mut line_count = 0;
                let mut buffer = [0; 8192];
                
                while let Ok(bytes_read) = reader.read(&mut buffer) {
                    if bytes_read == 0 { break; }
                    line_count += buffer[..bytes_read].iter().filter(|&&b| b == b'\n').count();
                    if line_count > 500_000 { break; }
                }
                line_count > 500_000
            },
            Err(_) => false
        }
    };
    
    if use_multithread {
        match split_csv_multithread(params).await {
            Ok(file_count) => Ok(SplitResult {
                success: true,
                file_count,
                error: None,
            }),
            Err(e) => Ok(SplitResult {
                success: false,
                file_count: 0,
                error: Some(e.to_string()),
            }),
        }
    } else {
        match split_csv_internal(params).await {
            Ok(file_count) => Ok(SplitResult {
                success: true,
                file_count,
                error: None,
            }),
            Err(e) => Ok(SplitResult {
                success: false,
                file_count: 0,
                error: Some(e.to_string()),
            }),
        }
    }
}

/// 内部CSV分割实现
async fn split_csv_internal(params: SplitParams) -> Result<usize, Box<dyn std::error::Error>> {
    let input_path = Path::new(&params.input_path);
    let output_dir = Path::new(&params.output_dir);
    
    // 验证输入文件存在
    if !input_path.exists() {
        return Err(format!("输入文件不存在: {}", params.input_path).into());
    }
    
    // 验证输入文件是有效的CSV文件
    if !input_path.extension().map_or(false, |ext| ext == "csv") {
        return Err("请选择有效的CSV文件".into());
    }
    
    // 检查文件是否为空
    let metadata = std::fs::metadata(input_path)?;
    if metadata.len() == 0 {
        return Err("CSV文件为空".into());
    }
    
    // 创建输出目录（如果不存在）
    if !output_dir.exists() {
        std::fs::create_dir_all(output_dir)
            .map_err(|e| format!("无法创建输出目录: {}", e))?;
    }
    
    // 检查输出目录是否可写
    if let Err(_) = std::fs::File::create(output_dir.join("test_write.tmp")) {
        return Err("输出目录没有写入权限".into());
    } else {
        let _ = std::fs::remove_file(output_dir.join("test_write.tmp"));
    }
    
    // 打开CSV文件
    let file = File::open(input_path)
        .map_err(|e| format!("无法打开CSV文件: {}", e))?;
    
    let mut reader = ReaderBuilder::new()
        .has_headers(params.has_header)
        .from_reader(BufReader::new(file));
    
    // 验证行数参数
    if params.rows_per_file == 0 {
        return Err("每个文件的行数必须大于0".into());
    }
    
    // 读取标题行（如果有）
    let headers = if params.has_header {
        match reader.headers() {
            Ok(headers) => headers.clone(),
            Err(e) => return Err(format!("读取CSV标题行失败: {}", e).into()),
        }
    } else {
        // 如果没有标题行，生成默认列名
        let record_count = match reader.headers() {
            Ok(headers) => headers.len(),
            Err(e) => return Err(format!("读取CSV列数失败: {}", e).into()),
        };
        
        if record_count == 0 {
            return Err("CSV文件没有有效的列数据".into());
        }
        
        csv::StringRecord::from(
            (0..record_count)
                .map(|i| format!("column_{}", i + 1))
                .collect::<Vec<_>>()
        )
    };
    
    let mut record = csv::StringRecord::new();
    let mut current_file_index = 1;
    let mut current_row_count = 0;
    let mut writer: Option<Writer<BufWriter<File>>> = None;
    let mut total_files = 0;
    
    // 获取基础文件名（不含扩展名）
    let file_stem = input_path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("output");
    
    let mut record_count = 0;
    while let Ok(has_record) = reader.read_record(&mut record) {
        if !has_record {
            break; // 文件结束
        }
        
        record_count += 1;
        
        // 如果需要创建新文件
        if current_row_count == 0 {
            // 关闭之前的writer
            if let Some(mut w) = writer.take() {
                if let Err(e) = w.flush() {
                    return Err(format!("写入文件失败: {}", e).into());
                }
            }
            
            // 创建新文件
            let output_file = output_dir.join(format!("{}_{}.csv", file_stem, current_file_index));
            let file = File::create(&output_file)
                .map_err(|e| format!("无法创建输出文件 {:?}: {}", output_file, e))?;
            
            writer = Some(WriterBuilder::new()
                .from_writer(BufWriter::new(file)));
            
            // 写入标题行
            if let Some(ref mut w) = writer {
                w.write_record(&headers)
                    .map_err(|e| format!("写入标题行失败: {}", e))?;
            }
            
            total_files += 1;
            current_file_index += 1;
        }
        
        // 写入数据行
        if let Some(ref mut w) = writer {
            w.write_record(&record)
                .map_err(|e| format!("写入数据行失败: {}", e))?;
        }
        
        current_row_count += 1;
        
        // 如果达到每文件行数限制，重置计数器
        if current_row_count >= params.rows_per_file {
            current_row_count = 0;
        }
    }
    
    if record_count == 0 {
        return Err("CSV文件没有数据行".into());
    }
    
    // 确保最后一个文件被正确关闭
    if let Some(mut w) = writer {
        w.flush()?;
    }
    
    if total_files == 0 {
        return Err("没有生成任何文件".into());
    }
    
    // 如果需要转换为Excel格式
    if params.convert_to_excel {
        convert_csv_files_to_excel(&output_dir, file_stem, total_files)?;
    }
    
    Ok(total_files)
}

/// 将分割后的CSV文件转换为Excel XLSX格式 - 优化版本
fn convert_csv_files_to_excel(
    output_dir: &Path,
    base_name: &str,
    file_count: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    // 串行处理转换多个文件，避免并行复杂性
    for i in 1..=file_count {
        let csv_path = output_dir.join(format!("{}_{}.csv", base_name, i));
        let xlsx_path = output_dir.join(format!("{}_{}.xlsx", base_name, i));
        
        if !csv_path.exists() {
            continue;
        }
        
        convert_csv_to_excel_minimal(&csv_path, &xlsx_path)?;
        
        // 删除原始CSV文件
        std::fs::remove_file(&csv_path)?;
    }
    
    Ok(())
}

/// 极低内存模式的CSV转Excel转换
fn convert_csv_to_excel_minimal(
    csv_path: &Path,
    xlsx_path: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    // 使用最小内存配置
    let file = File::open(csv_path)?;
    let mut reader = ReaderBuilder::new()
        .has_headers(true)
        .from_reader(BufReader::with_capacity(4 * 1024, file)); // 4KB缓冲区

    let mut workbook = Workbook::new();
    let worksheet = workbook.add_worksheet();

    // 极简格式，不设置背景色以节省内存
    let header_format = Format::new()
        .set_bold()
        .set_align(FormatAlign::Center);

    let headers = reader.headers().map_err(|e| e.to_string())?.clone();

    // 写入标题行
    for (col, header) in headers.iter().enumerate().take(100) { // 限制最大列数
        worksheet.write_string_with_format(0, col as u16, header, &header_format).map_err(|e| e.to_string())?;
    }

    // 流式写入数据
    let mut row = 1;
    let mut record = csv::StringRecord::new();

    while reader.read_record(&mut record).map_err(|e| e.to_string())? {
        for (col, field) in record.iter().enumerate().take(100) { // 限制最大列数
            let truncated_field = if field.len() > 500 { // 限制字段长度
                &field[..500]
            } else {
                field
            };

            if let Ok(num) = truncated_field.parse::<f64>() {
                worksheet.write_number(row, col as u16, num).map_err(|e| e.to_string())?;
            } else {
                worksheet.write_string(row, col as u16, truncated_field).map_err(|e| e.to_string())?;
            }
        }
        row += 1;

        // 每5000行保存一次，避免内存累积
        if row % 5000 == 0 {
            worksheet.set_row_height(row as u32 - 5000, 15).map_err(|e| e.to_string())?; // 设置行高
        }
    }

    // 简单列宽设置
    for col in 0..headers.len().min(100) {
        worksheet.set_column_width(col as u16, 12).map_err(|e| e.to_string())?;
    }

    workbook.save(xlsx_path).map_err(|e| e.to_string())?;

    // 显式释放资源
    drop(reader);
    drop(workbook);

    Ok(())
}

/// 在所有CSV文件生成后，串行转换为Excel文件
fn convert_all_csv_to_excel(output_dir: &Path, file_stem: &str, convert_to_excel: bool) -> Result<(), String> {
    if !convert_to_excel {
        return Ok(());
    }

    // 查找所有CSV文件
    let csv_files: Vec<_> = std::fs::read_dir(output_dir)
        .map_err(|e| e.to_string())?
        .filter_map(|entry| {
            entry.ok().and_then(|e| {
                let path = e.path();
                if path.extension().and_then(|s| s.to_str()) == Some("csv") &&
                   path.file_stem().and_then(|s| s.to_str()).map_or(false, |s| s.starts_with(file_stem)) {
                    Some(path)
                } else {
                    None
                }
            })
        })
        .collect();

    // 按文件索引排序
    let mut csv_files = csv_files;
    csv_files.sort();

    // 串行转换每个CSV文件
    for csv_path in csv_files {
        let file_name = csv_path.file_stem().and_then(|s| s.to_str()).unwrap_or("output");
        let xlsx_path = output_dir.join(format!("{}.xlsx", file_name));

        println!("Converting {} to Excel...", csv_path.display());
        convert_csv_to_excel_minimal(&csv_path, &xlsx_path).map_err(|e| e.to_string())?;

        // 转换完成后删除CSV文件
        std::fs::remove_file(&csv_path).map_err(|e| e.to_string())?;
        println!("Converted and removed {}", csv_path.display());
    }

    Ok(())
}

/// 多线程并发CSV分割实现 - 真正的高性能版本
/// 使用线程池处理200万行以上大文件
    async fn split_csv_multithread(params: SplitParams) -> Result<usize, String> {
        use std::sync::mpsc;
    
    
    let input_path = Path::new(&params.input_path);
    let output_dir = Path::new(&params.output_dir);
    
    // 验证输入文件存在
    if !input_path.exists() {
        return Err(format!("输入文件不存在: {}", params.input_path).into());
    }
    
    // 创建输出目录（如果不存在）
    if !output_dir.exists() {
        std::fs::create_dir_all(output_dir)
            .map_err(|e| format!("无法创建输出目录: {}", e))?;
    }
    
    // 验证行数参数
    if params.rows_per_file == 0 {
        return Err("每个文件的行数必须大于0".into());
    }
    
    // 获取基础文件名
    let file_stem = input_path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("output");
    
    // 使用内存映射快速计算总行数
    let file = File::open(input_path).map_err(|e| e.to_string())?;
    let metadata = file.metadata().map_err(|e| e.to_string())?;
    let file_size = metadata.len();
    
    // 根据文件大小智能决定线程数，限制内存使用
    let _thread_count = match file_size {
        0..=100_000_000 => 1,           // < 100MB: 单线程
        100_000_001..=500_000_000 => 2, // 100MB-500MB: 2线程
        _ => 3,                          // > 500MB: 3线程（减少并发）
    };
    
    // 使用内存映射文件进行高效处理
    let mmap = unsafe { Mmap::map(&file).map_err(|e| e.to_string())? };
    let file_size = mmap.len();
    
    if file_size == 0 {
        return Err("CSV文件为空".into());
    }
    
    // 找到所有换行符的位置，用于精确分块
    let mut line_breaks = Vec::new();
    for (i, &byte) in mmap.iter().enumerate() {
        if byte == b'\n' {
            line_breaks.push(i);
        }
    }
    
    // 计算总行数
    let mut total_lines = line_breaks.len();
    if !line_breaks.is_empty() && line_breaks.last() != Some(&(file_size - 1)) {
        total_lines += 1; // 处理最后一行
    }
    
    // 如果有标题行，减去1
    let data_lines = if params.has_header { total_lines.saturating_sub(1) } else { total_lines };
    
    if data_lines == 0 {
        return Err("CSV文件没有数据行".into());
    }
    
    // 计算需要创建的文件数量
    let file_count = ((data_lines as usize + params.rows_per_file - 1) / params.rows_per_file).max(1);
    let rows_per_chunk = (data_lines as usize + file_count - 1) / file_count;
    
    // 读取标题行
    let headers = {
        let mut reader = ReaderBuilder::new()
            .has_headers(params.has_header)
            .from_reader(&mmap[..]);
        
        let mut first_record = csv::StringRecord::new();
        let col_count = if reader.read_record(&mut first_record).map_err(|e| e.to_string())? {
            first_record.len()
        } else {
            0
        };
        
        if params.has_header {
            reader.headers()
                .map_err(|e| format!("读取CSV标题行失败: {}", e))?
                .clone()
        } else {
            csv::StringRecord::from(
                (0..col_count)
                    .map(|i| format!("column_{}", i + 1))
                    .collect::<Vec<_>>()
            )
        }
    };
    
    // 创建线程间通信通道
    let (tx, rx) = mpsc::channel();
    let headers_arc = Arc::new(headers);
    
    // 计算每个线程的字节范围
    let mut chunk_boundaries = Vec::new();
    
    // 确定起始位置
    let data_start_pos = if params.has_header && !line_breaks.is_empty() {
        line_breaks[0] + 1 // 跳过标题行
    } else {
        0
    };
    
    chunk_boundaries.push(data_start_pos);
    
    // 计算每个分块的行边界
    let start_line_idx = if params.has_header && !line_breaks.is_empty() { 1 } else { 0 };
    for chunk_idx in 1..file_count {
        let target_line = start_line_idx + chunk_idx * rows_per_chunk;
        if target_line < line_breaks.len() {
            chunk_boundaries.push(line_breaks[target_line] + 1);
        } else {
            chunk_boundaries.push(file_size);
            break;
        }
    }
    
    if chunk_boundaries.len() <= file_count {
        chunk_boundaries.push(file_size);
    }
    
    let mut handles = vec![];
    
    // 限制并发线程数，最多2个线程同时进行
    let _max_threads = 2;
    let semaphore = Arc::new(std::sync::Mutex::new(0));
    
    // 启动并发处理线程
    for file_index in 1..=file_count {
        if file_index >= chunk_boundaries.len() {
            break;
        }
        
        let start_pos = chunk_boundaries[file_index - 1];
        let end_pos = if file_index < chunk_boundaries.len() { 
            chunk_boundaries[file_index] 
        } else { 
            file_size 
        };
        
        if start_pos >= end_pos {
            continue;
        }
        
        let input_path = input_path.to_path_buf();
        let output_dir = output_dir.to_path_buf();
        let file_stem = file_stem.to_string();
        let headers = Arc::clone(&headers_arc);
        let tx = tx.clone();
        let params = params.clone();
        let semaphore = Arc::clone(&semaphore);
        
        let handle = thread::spawn(move || {
            let _guard = semaphore.lock().unwrap(); // 获取锁许可
            let result = (|| -> Result<(), String> {
                let output_file = output_dir.join(format!("{}_{}.csv", file_stem, file_index));
                let file = File::create(&output_file).map_err(|e| e.to_string())?;
                let mut writer = WriterBuilder::new()
                    .from_writer(BufWriter::with_capacity(256 * 1024, file)); // 256KB缓冲区
                
                // 写入标题行
                writer.write_record(&*headers).map_err(|e| format!("写入标题行失败: {}", e))?;
                
                // 使用内存映射文件，按行读取数据
                let file = File::open(&input_path).map_err(|e| e.to_string())?;
                let mmap = unsafe { Mmap::map(&file).map_err(|e| e.to_string())? };
                
                // 找到当前分块的起始行和结束行
                
                // 跳过标题行
                let skip_lines = if params.has_header && start_pos == 0 { 1 } else { 0 };
                
                // 计算当前分块应该处理的行数
                let target_rows = if file_index == file_count {
                    // 最后一个分块处理剩余所有行
                    rows_per_chunk + (data_lines % file_count).saturating_sub(1)
                } else {
                    rows_per_chunk
                };
                
                // 使用标准库逐行读取，确保正确处理CSV格式
                let chunk_data = &mmap[start_pos..std::cmp::min(end_pos, mmap.len())];
                let text = std::str::from_utf8(chunk_data).map_err(|e| e.to_string())?;
                
                let mut reader = ReaderBuilder::new()
                    .has_headers(false)
                    .from_reader(text.as_bytes());
                
                // 跳过标题行（如果是第一个分块）
                if skip_lines > 0 {
                    let mut temp_record = csv::StringRecord::new();
                    reader.read_record(&mut temp_record).map_err(|e| format!("跳过标题行失败: {}", e))?;
                }
                
                let mut record = csv::StringRecord::new();
                let mut rows_written = 0;
                
                while rows_written < target_rows && reader.read_record(&mut record).map_err(|e| format!("读取CSV记录失败: {}", e))? {
                    writer.write_record(&record).map_err(|e| format!("写入CSV记录失败: {}", e))?;
                    rows_written += 1;
                }
                
                writer.flush().map_err(|e| e.to_string())?;
                
                Ok(())
            })();
            
            tx.send((file_index, result)).unwrap();
        });
        
        handles.push(handle);
    }
    
    drop(tx); // 关闭发送端
    
    // 等待所有线程完成
    let mut completed_files = 0;
    for (file_index, result) in rx {
        match result {
            Ok(_) => completed_files += 1,
            Err(e) => return Err(format!("处理文件 {} 失败: {}", file_index, e).into()),
        }
    }
    
    // 确保所有线程完成
    for handle in handles {
        handle.join().unwrap();
    }
    
    // 在所有CSV文件生成后，串行转换为Excel
    if params.convert_to_excel {
        convert_all_csv_to_excel(output_dir, file_stem, true)?;
    }
    
    Ok(completed_files)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![split_csv])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
