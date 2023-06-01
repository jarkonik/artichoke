# frozen_string_literal: true

# Tests from Kernel core docs in Ruby 2.6.3
# https://ruby-doc.org/core-2.6.3/IO.html
def spec
  binread_testfile
  binread_testfile_with_length
  binread_testfile_with_length_and_offset

  true
end

def binread_testfile
  raise unless IO.binread("testfile") == "This is line one\nThis is line two\nThis is line three\nAnd so on...\n"
end

def binread_testfile_with_length
  raise unless IO.binread("testfile", 20) == "This is line one\nThi"
end

def binread_testfile_with_length_and_offset
  raise unless IO.binread("testfile", 20, 10) == "ne one\nThis is line "
end



