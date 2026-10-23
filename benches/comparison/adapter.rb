require 'json'
parser, action, profile, path = ARGV
abort 'Usage: adapter.rb PARSER bench|render PROFILE INPUT.json' unless ARGV.length == 4 && %w[bench render].include?(action) && %w[markly commonmarker inkmark].include?(parser)
require parser
if parser == 'inkmark'
  if profile == 'default'
    render = ->(s) { Inkmark.to_html(s) }
  else
    options = {preset: profile.start_with?('gfm') ? :gfm : :commonmark, raw_html: true}
    if profile.start_with?('gfm')
      options[:footnotes] = profile == 'gfm-footnotes'
      options[:links] = {autolink: true}
    end
    render = ->(s) { Inkmark.to_html(s, options: options) }
  end
elsif parser == 'markly'
  flags = profile == 'default' ? Markly::DEFAULT : Markly::UNSAFE
  flags |= Markly::FOOTNOTES if profile == 'gfm-footnotes'
  extensions = profile.start_with?('gfm') ? [:table, :strikethrough, :autolink, :tagfilter, :tasklist] : []
  render = ->(s) { Markly.render_html(s, flags: flags, extensions: extensions) }
else
  if profile == 'default'
    render = ->(s) { Commonmarker.to_html(s) }
  else
    extension = {strikethrough: false, table: false, autolink: false, tasklist: false, shortcodes: false, header_ids: nil}
    extension.merge!({strikethrough: true, table: true, autolink: true, tasklist: true}) if profile.start_with?('gfm')
    extension[:footnotes] = true if profile == 'gfm-footnotes'
    options = {render: {hardbreaks: false, github_pre_lang: false, escaped_char_spans: false, unsafe: true}, extension: extension}
    render = ->(s) { Commonmarker.to_html(s, options: options, plugins: {syntax_highlighter: nil}) }
  end
end
cases = JSON.parse(File.read(path, encoding: 'UTF-8'))
if action == 'render'
  puts JSON.generate(cases.map {|c| render.call(c.fetch('markdown'))})
else
  clock = -> { Process.clock_gettime(Process::CLOCK_MONOTONIC) }
  cases.each do |c|
    s = c.fetch('markdown')
    start = clock.call; warm = 0
    while clock.call - start < 0.2
      render.call(s); warm += 1
    end
    n = [(warm / (clock.call - start) * 0.15).ceil, 1].max
    times = 7.times.map do
      start = clock.call
      n.times { render.call(s) }
      (clock.call - start) / n
    end.sort
    puts JSON.generate({parser: parser, profile: profile, document: c.fetch('name'), bytes: s.bytesize, iterations_per_sample: n, samples_seconds: times, median_seconds: times[3], mib_per_second: s.bytesize / times[3] / 1048576.0})
    STDOUT.flush
  end
end
